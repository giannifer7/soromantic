use crate::db::Database;
use libc::size_t;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Arc;

pub struct CContext {
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub db: Arc<Database>,
    pub mpv: Arc<crate::mpv::MpvClient>,
}

macro_rules! check_null {
    (void, $($ptr:expr),+) => {
        if $($ptr.is_null())||+ {
            return;
        }
    };
    ($ret:expr, $($ptr:expr),+) => {
        if $($ptr.is_null())||+ {
            return $ret;
        }
    };
}

macro_rules! get_context {
    ($ctx:expr) => {{
        let ctx = unsafe { &*$ctx };
        (Arc::clone(&ctx.runtime), Arc::clone(&ctx.db))
    }};
}

macro_rules! process_result {
    ($rt:expr, $future:expr, $out_total:expr, $out_len:expr, $converter:expr) => {
        $rt.block_on(async {
            match $future.await {
                Ok((items, total)) => {
                    if !$out_total.is_null() {
                        unsafe { *$out_total = total };
                    }
                    unsafe { *$out_len = items.len() };
                    $converter(items)
                }
                Err(e) => {
                    tracing::error!("FFI error: {}", e);
                    unsafe { *$out_len = 0 };
                    std::ptr::null_mut()
                }
            }
        })
    };
}

macro_rules! execute_paginated_query {
    ($ctx:expr, $out_total:expr, $out_len:expr, |$db:ident| $future:expr, $converter:expr) => {{
        check_null!(std::ptr::null_mut(), $ctx, $out_len);
        let (rt, $db) = get_context!($ctx);
        process_result!(rt, $future, $out_total, $out_len, $converter)
    }};
}

fn string_to_c_char(s: &str) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

#[repr(C)]
pub struct CVideoEntry {
    pub id: i64,
    pub title: *mut c_char,
    pub local_image: *mut c_char,
    pub local_preview: *mut c_char,
    pub finished_videos: i64,
    pub failed_videos: i64,
}

#[repr(C)]
pub struct CPageData {
    pub id: i64,
    pub title: *mut c_char,
    pub local_image: *mut c_char,
    pub studio: *mut c_char,
    // Removed grid and grid_count as they are now paginated separately
}

/// Initialize the library.
/// Returns a pointer to the context, or NULL on failure.
///
/// # Safety
/// `db_path` must be a valid C-string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_init(
    db_path: *const c_char,
    config_path: *const c_char,
) -> *mut CContext {
    // Initialize logging if not already done
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    tracing::info!("Initializing Soromantic Core FFI...");
    let start = std::time::Instant::now();

    let path_str = if db_path.is_null() {
        String::new()
    } else {
        // SAFETY: We checked for null, and we trust the caller to pass a valid C-string.
        unsafe { CStr::from_ptr(db_path) }
            .to_string_lossy()
            .to_string()
    };

    let config_path_str = if config_path.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(config_path) }
            .to_string_lossy()
            .to_string()
    };

    let Ok(rt) = tokio::runtime::Runtime::new() else {
        return std::ptr::null_mut();
    };

    // We block here to initialize the DB
    let Some(db) = rt.block_on(async move {
        tracing::info!("FFI: soromantic_init v2 starting...");
        // Load config
        // If specific config path provided and exists, use it. Otherwise default.
        let config_arg = if config_path_str.is_empty() {
            None
        } else {
            let p = std::path::Path::new(&config_path_str);
            if p.exists() {
                Some(config_path_str.as_str())
            } else {
                None
            }
        };

        let config_status = crate::config::load_config(config_arg).ok()?;

        let mut config = match config_status {
            crate::config::ConfigStatus::Loaded(cfg) => *cfg,
            crate::config::ConfigStatus::Created(_) => {
                // Config was just created, load it now
                match crate::config::load_config(config_arg).ok()? {
                    crate::config::ConfigStatus::Loaded(cfg) => *cfg,
                    crate::config::ConfigStatus::Created(_) => return None,
                }
            }
        };

        if !path_str.is_empty() {
            config.db_path = std::path::PathBuf::from(path_str);
        }

        Database::new(config).await.ok()
    }) else {
        return std::ptr::null_mut();
    };

    tracing::info!("Database initialized in {:?}", start.elapsed());

    let mpv = crate::mpv::MpvClient::new_unix(
        db.config.mpv_socket.to_string_lossy().to_string(),
        db.config.timeouts.mpv_socket_connect,
        db.config.timeouts.mpv_socket_command,
    );

    let ctx = Box::new(CContext {
        runtime: Arc::new(rt),
        db: Arc::new(db),
        mpv: Arc::new(mpv),
    });

    Box::into_raw(ctx)
}

/// Free the library context.
///
/// # Safety
/// The `ctx` pointer must be a valid pointer obtained from `soromantic_init`.
/// After calling this function, the pointer is invalid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_free_context(ctx: *mut CContext) {
    check_null!(void, ctx);
    unsafe {
        let _ = Box::from_raw(ctx);
    }
}

/// Get the database busy timeout from config (in milliseconds).
///
/// # Safety
/// The `ctx` pointer must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_db_busy_timeout(ctx: *mut CContext) -> u64 {
    check_null!(crate::config::DEFAULT_DB_BUSY_TIMEOUT_MS, ctx); // Default fallback
    let (_, db) = get_context!(ctx);
    db.config.timeouts.db_busy
}

/// Get the database path from config.
/// Returns a string that must be freed with `soromantic_free_cstring`.
///
/// # Safety
/// The `ctx` pointer must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_db_path(ctx: *mut CContext) -> *mut c_char {
    check_null!(std::ptr::null_mut(), ctx);
    let (_, db) = get_context!(ctx);
    string_to_c_char(&db.config.db_path.to_string_lossy())
}

/// Get the download delay from config (in milliseconds).
///
/// # Safety
/// The `ctx` pointer must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_download_delay_ms(ctx: *mut CContext) -> u64 {
    check_null!(0, ctx);
    let (_, db) = get_context!(ctx);
    db.config.download_delay_ms
}

/// # Safety
/// `ctx` must be a valid context pointer.
/// `ids` must be a valid pointer to an array of `i64` with length `len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_play_playlist(
    ctx: *mut CContext,
    ids: *const i64,
    len: size_t,
) {
    check_null!(void, ctx, ids);
    if len == 0 {
        println!("FFI: soromantic_play_playlist called with 0 ids, returning");
        return;
    }

    let ctx_ref = unsafe { &*ctx };

    let id_slice = unsafe { std::slice::from_raw_parts(ids, len) };
    println!("FFI: soromantic_play_playlist called with {len} ids: {id_slice:?}");
    let id_vec = id_slice.to_vec();

    let db = Arc::clone(&ctx_ref.db);
    let mpv = Arc::clone(&ctx_ref.mpv);

    ctx_ref.runtime.spawn(async move {
        match db.get_playlist(&id_vec).await {
            Ok(playlist) => {
                println!("FFI: get_playlist returned {} items", playlist.len());
                for item in &playlist {
                    println!(
                        "FFI:   playlist item: title={}, path={}",
                        item.title, item.path
                    );
                }
                if playlist.is_empty() {
                    eprintln!("FFI: Playlist is EMPTY! No videos found for these IDs.");
                    return;
                }
                if let Err(e) = mpv.play_playlist(&playlist) {
                    eprintln!("FFI: Failed to play playlist: {e}");
                } else {
                    println!("FFI: play_playlist succeeded");
                }
            }
            Err(e) => eprintln!("FFI: Failed to get playlist: {e}"),
        }
    });
}

/// Search for videos.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `query` must be a valid C-string.
/// `out_count` must be a valid pointer to write the result count.
/// The returned pointer array must be freed with `soromantic_free_library` if not null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_search(
    ctx: *mut CContext,
    query: *const c_char,
    limit: size_t,
    out_count: *mut size_t,
) -> *mut CVideoEntry {
    check_null!(std::ptr::null_mut(), ctx, query, out_count);
    let (rt, db) = get_context!(ctx);

    let q_str = unsafe { std::ffi::CStr::from_ptr(query) }
        .to_string_lossy()
        .into_owned();

    tracing::info!("FFI: Searching for '{}' (limit: {})", q_str, limit);

    rt.block_on(async move {
        #[allow(clippy::cast_possible_wrap)]
        match db.search_pages(&q_str, limit as i64).await {
            Ok(items) => {
                let count = items.len();
                unsafe { *out_count = count };
                if count == 0 {
                    return std::ptr::null_mut();
                }

                convert_library_items_to_c(items)
            }
            Err(e) => {
                tracing::error!("FFI: Search failed: {}", e);
                unsafe { *out_count = 0 };
                std::ptr::null_mut()
            }
        }
    })
}

/// Helper to convert a list of `LibraryItems` to `CVideoEntry` array
fn convert_library_items_to_c(items: Vec<crate::db::LibraryItem>) -> *mut CVideoEntry {
    let mut c_items = Vec::with_capacity(items.len());
    for item in items {
        c_items.push(CVideoEntry {
            id: item.id,
            title: CString::new(item.title).unwrap_or_default().into_raw(),
            local_image: item.local_image.map_or(std::ptr::null_mut(), |s| {
                CString::new(s).unwrap_or_default().into_raw()
            }),
            local_preview: item.local_preview.map_or(std::ptr::null_mut(), |s| {
                CString::new(s).unwrap_or_default().into_raw()
            }),
            finished_videos: item.finished_videos,
            failed_videos: item.failed_videos,
        });
    }

    let ptr = c_items.as_mut_ptr();
    std::mem::forget(c_items); // Prevent Vec from freeing memory
    ptr
}

/// Get paginated library items.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `out_len` must be a valid pointer to write the item count.
/// If `out_total` is not null, it must be a valid pointer to write the total count.
/// The returned pointer array must be freed with `soromantic_free_library` if not null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_library(
    ctx: *mut CContext,
    offset: i64,
    limit: i64,
    out_total: *mut i64,
    out_len: *mut size_t,
) -> *mut CVideoEntry {
    check_null!(std::ptr::null_mut(), ctx, out_len);
    let (rt, db) = get_context!(ctx);

    let start = std::time::Instant::now();
    let future = db.get_library_paginated(offset, limit, false);
    let duration = start.elapsed();
    tracing::info!(
        "Query Offset: {}, Limit: {}, Total: ? -> took {:?}",
        offset,
        limit,
        duration
    );

    process_result!(rt, future, out_total, out_len, convert_library_items_to_c)
}

/// Free a library entry array.
///
/// # Safety
/// `entries` must point to a valid array of `CVideoEntry` with length `len`,
/// allocated by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_free_library(entries: *mut CVideoEntry, len: size_t) {
    check_null!(void, entries);
    unsafe {
        let slice = std::slice::from_raw_parts_mut(entries, len);
        for entry in slice.iter() {
            if !entry.title.is_null() {
                let _ = CString::from_raw(entry.title);
            }
            if !entry.local_image.is_null() {
                let _ = CString::from_raw(entry.local_image);
            }
            if !entry.local_preview.is_null() {
                let _ = CString::from_raw(entry.local_preview);
            }
        }
        let _ = Box::from_raw(slice);
    }
}

/// Get the configured previews directory.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// The returned string must be freed with `soromantic_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_previews_dir(ctx: *mut CContext) -> *mut c_char {
    check_null!(std::ptr::null_mut(), ctx);
    let (_, db) = get_context!(ctx);
    CString::new(db.config.previews_dir.to_string_lossy().to_string())
        .unwrap_or_default()
        .into_raw()
}

/// Get the configured cache directory.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// The returned string must be freed with `soromantic_free_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_cache_dir(ctx: *mut CContext) -> *mut c_char {
    check_null!(std::ptr::null_mut(), ctx);
    let (_, db) = get_context!(ctx);
    CString::new(db.config.cache_dir.to_string_lossy().to_string())
        .unwrap_or_default()
        .into_raw()
}

/// Ensure previews exist for a video.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `preview_path` must be a valid C-string.
/// `out_len` must be a valid pointer to write the count.
/// The returned array of strings must be freed with `soromantic_free_strings`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_ensure_previews(
    ctx: *mut CContext,
    id: i64,
    preview_path: *const c_char,
    out_len: *mut size_t,
) -> *mut *mut c_char {
    check_null!(std::ptr::null_mut(), ctx, preview_path, out_len);
    let (_, db) = get_context!(ctx);
    let file_path = unsafe { CStr::from_ptr(preview_path) }.to_string_lossy();

    let frames = crate::previews::ensure_preview_frames(
        id,
        std::path::Path::new(&*file_path),
        &db.config.cache_dir.join("previews"),
    );

    match frames {
        Ok(paths) => {
            unsafe { *out_len = paths.len() };
            let mut c_paths = Vec::with_capacity(paths.len());
            for p in paths {
                c_paths.push(
                    CString::new(p.to_string_lossy().to_string())
                        .unwrap_or_default()
                        .into_raw(),
                );
            }
            let mut slice = c_paths.into_boxed_slice();
            let ptr = slice.as_mut_ptr();
            std::mem::forget(slice);
            ptr
        }
        Err(e) => {
            tracing::warn!("FFI: Failed to ensure previews for {}: {}", id, e);
            unsafe { *out_len = 0 };
            std::ptr::null_mut()
        }
    }
}

/// Get all cached library items.
///
/// # Safety
/// `out_len` must be a valid pointer to write the item count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_cache(out_len: *mut size_t) -> *mut CVideoEntry {
    check_null!(std::ptr::null_mut(), out_len);

    // Try to load config to find cache dir
    let config = match crate::config::load_config(None).ok() {
        Some(crate::config::ConfigStatus::Loaded(cfg)) => *cfg,
        _ => return std::ptr::null_mut(),
    };

    let Some(items) = crate::cache::load_library_cache(&config.cache_dir) else {
        unsafe { *out_len = 0 };
        return std::ptr::null_mut();
    };

    unsafe { *out_len = items.len() };

    unsafe { *out_len = items.len() };
    convert_library_items_to_c(items)
}

/// Update the library cache with new items.
///
/// # Safety
/// `entries` must point to a valid array of `CVideoEntry` with length `len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_update_cache(entries: *mut CVideoEntry, len: size_t) {
    check_null!(void, entries);
    if len == 0 {
        return;
    }

    // Try to load config to find cache dir
    let config = match crate::config::load_config(None).ok() {
        Some(crate::config::ConfigStatus::Loaded(cfg)) => *cfg,
        _ => return,
    };

    let items = unsafe {
        let v = std::slice::from_raw_parts(entries, len);
        v.iter()
            .map(|e| crate::db::LibraryItem {
                id: e.id,
                title: if e.title.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(e.title).to_string_lossy().into_owned()
                },
                local_image: if e.local_image.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(e.local_image).to_string_lossy().into_owned())
                },
                local_preview: if e.local_preview.is_null() {
                    None
                } else {
                    Some(
                        CStr::from_ptr(e.local_preview)
                            .to_string_lossy()
                            .into_owned(),
                    )
                },
                finished_videos: e.finished_videos,
                failed_videos: e.failed_videos,
                ..Default::default()
            })
            .collect::<Vec<_>>()
    };

    if let Err(e) = crate::cache::write_library_cache(&config.cache_dir, &items) {
        tracing::error!("FFI: Failed to write library cache: {}", e);
    }
}

/// Free a single string.
///
/// # Safety
/// `s` must be a valid pointer to a C-string allocated by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

/// Free an array of strings.
///
/// # Safety
/// `arr` must point to a valid array of C-strings with length `len`,
/// allocated by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_free_strings(arr: *mut *mut c_char, len: size_t) {
    check_null!(void, arr);
    unsafe {
        let slice = std::slice::from_raw_parts_mut(arr, len);
        for s in slice.iter() {
            if !s.is_null() {
                let _ = CString::from_raw(*s);
            }
        }
        let _ = Box::from_raw(slice);
    }
}

/// Get detailed info for a page.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// The returned struct must be freed with `soromantic_free_page_data`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_page_info(ctx: *mut CContext, id: i64) -> *mut CPageData {
    check_null!(std::ptr::null_mut(), ctx);
    let (rt, db) = get_context!(ctx);

    rt.block_on(async {
        match db.get_page_info(id).await {
            Ok(Some(page)) => Box::into_raw(Box::new(CPageData {
                id: page.id,
                title: string_to_c_char(&page.title),
                local_image: string_to_c_char(&page.local_image.unwrap_or_default()),
                studio: string_to_c_char(&page.studio.unwrap_or_default()),
            })),
            _ => std::ptr::null_mut(),
        }
    })
}

/// Free page data structure.
///
/// # Safety
/// `p` must be a valid pointer to `CPageData` allocated by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_free_page_data(p: *mut CPageData) {
    check_null!(void, p);
    unsafe {
        let p = Box::from_raw(p);
        soromantic_free_string(p.title);
        soromantic_free_string(p.local_image);
        soromantic_free_string(p.studio);
    }
}

/// Get related items with pagination.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `out_len` must be a valid pointer to write the count.
/// If `out_total` is not null, it must be a valid pointer to write the total count.
/// The returned array must be freed with `soromantic_free_library`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_related(
    ctx: *mut CContext,
    id: i64,
    offset: i64,
    limit: i64,
    out_total: *mut i64,
    out_len: *mut size_t,
) -> *mut CVideoEntry {
    // get_related_paginated returns Result<(items, total)>
    execute_paginated_query!(
        ctx,
        out_total,
        out_len,
        |db| db.get_related_paginated(id, offset, limit),
        convert_library_items_to_c
    )
}

/// Get all related items.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `out_len` must be a valid pointer to write the item count.
/// The returned array must be freed with `soromantic_free_library`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_related_all(
    ctx: *mut CContext,
    id: i64,
    out_len: *mut size_t,
) -> *mut CVideoEntry {
    check_null!(std::ptr::null_mut(), ctx, out_len);
    let (rt, db) = get_context!(ctx);

    rt.block_on(async {
        if let Ok(items) = db.get_related(id).await {
            unsafe { *out_len = items.len() };

            let mut c_items = Vec::with_capacity(items.len());
            for item in items {
                c_items.push(CVideoEntry {
                    id: item.id.unwrap_or(0),
                    title: string_to_c_char(&item.title),
                    local_image: string_to_c_char(&item.local_image.unwrap_or_default()),
                    local_preview: string_to_c_char(&item.local_preview.unwrap_or_default()),
                    finished_videos: item.finished_videos,
                    failed_videos: item.failed_videos,
                });
            }
            let ptr = c_items.as_mut_ptr();
            std::mem::forget(c_items);
            ptr
        } else {
            unsafe { *out_len = 0 };
            std::ptr::null_mut()
        }
    })
}

#[repr(C)]
pub struct CPerformerItem {
    pub id: i64,
    pub name: *mut c_char,
    pub star: i32,
    pub sex: i32,
    pub birth_year: i32,
    pub aliases: *mut c_char,
    pub thumb_status: i64,
    pub nation_id: i64,
    pub count: i64,
}

fn convert_performers_to_c(items: Vec<crate::db::PerformerItem>) -> *mut CPerformerItem {
    let mut c_items = Vec::with_capacity(items.len());
    for item in items {
        c_items.push(CPerformerItem {
            id: item.id,
            name: string_to_c_char(&item.name),
            star: item.star,
            sex: item.sex,
            birth_year: item.birth_year.unwrap_or(0),
            aliases: string_to_c_char(&item.aliases.unwrap_or_default()),
            thumb_status: item.thumb_status,
            nation_id: item.nation_id.unwrap_or(0),
            count: item.count,
        });
    }

    let ptr = c_items.as_mut_ptr();
    std::mem::forget(c_items); // Prevent Vec from freeing memory
    ptr
}

/// Get paginated performers.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `out_len` must be a valid pointer to write the item count.
/// If `out_total` is not null, it must be a valid pointer to write the total count.
/// The returned array must be freed with `soromantic_free_performers`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_performers(
    ctx: *mut CContext,
    offset: i64,
    limit: i64,
    search_query: *const c_char,
    out_total: *mut i64,
    out_len: *mut size_t,
) -> *mut CPerformerItem {
    let search_q = if search_query.is_null() {
        None
    } else {
        match unsafe { CStr::from_ptr(search_query) }.to_string_lossy() {
            qs if qs.is_empty() => None,
            qs => Some(qs.to_string()),
        }
    };

    execute_paginated_query!(
        ctx,
        out_total,
        out_len,
        |db| db.get_performers_paginated(offset, limit, search_q),
        convert_performers_to_c
    )
}

/// Free a performer array.
///
/// # Safety
/// `items` must point to a valid array of `CPerformerItem` with length `len`,
/// allocated by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_free_performers(items: *mut CPerformerItem, len: size_t) {
    check_null!(void, items);
    unsafe {
        let slice = std::slice::from_raw_parts_mut(items, len);
        for item in slice.iter() {
            soromantic_free_string(item.name);
            soromantic_free_string(item.aliases);
        }
        let _ = Box::from_raw(slice);
    }
}

/// Get paginated scenes for a performer.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `out_len` must be a valid pointer to write the item count.
/// If `out_total` is not null, it must be a valid pointer to write the total count.
/// The returned array must be freed with `soromantic_free_library`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_get_performer_scenes(
    ctx: *mut CContext,
    performer_id: i64,
    offset: i64,
    limit: i64,
    out_total: *mut i64,
    out_len: *mut size_t,
) -> *mut CVideoEntry {
    execute_paginated_query!(
        ctx,
        out_total,
        out_len,
        |db| db.get_videos_by_performer_paginated(performer_id, offset, limit, false),
        convert_library_items_to_c
    )
}

/// Upsert a nation.
/// Returns the nation ID on success, or -1 on failure.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `code` must be a valid C-string.
/// `name` may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_upsert_nation(
    ctx: *mut CContext,
    code: *const c_char,
    name: *const c_char,
) -> i64 {
    check_null!(-1, ctx, code);
    let (rt, db) = get_context!(ctx);

    let code_str = unsafe { CStr::from_ptr(code) }.to_string_lossy();
    let name_str = if name.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(name) }
                .to_string_lossy()
                .into_owned(),
        )
    };

    rt.block_on(async {
        match db.upsert_nation(&code_str, name_str.as_deref()).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("FFI: upsert_nation failed: {}", e);
                -1
            }
        }
    })
}

/// Run video scraper by ID.
/// Returns 0 on success, < 0 on failure.
///
/// # Safety
/// `ctx` must be a valid context pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_run_video_scraper_by_id(
    ctx: *mut CContext,
    id: i64,
    callback: Option<CProgressCallback>,
) -> i64 {
    check_null!(-1, ctx);
    let (rt, db) = get_context!(ctx);

    // Create config from db.config
    let config = crate::model_workflow::WorkflowConfig {
        models_dir: db.config.models_dir.clone(),
        flags_dir: db.config.flags_dir.clone(),
        covers_dir: db.config.covers_dir.clone(),
        thumbs_dir: db.config.thumbs_dir.clone(),
        previews_dir: db.config.previews_dir.clone(),
        scrapers_dir: db.config.scrapers_dir.clone(),
        download_delay_ms: db.config.download_delay_ms,
        ffmpeg_path: db.config.ffmpeg_path.clone(),
        ffprobe_path: db.config.ffprobe_path.clone(),
    };

    let db_clone = db.clone();

    // Wrap callback
    let on_progress: Option<crate::scripting::WorkflowProgressCallback> = callback.map(|cb| {
        std::sync::Arc::new(move |step: &str, msg: &str| {
            let s_step = std::ffi::CString::new(step).unwrap_or_default();
            let s_msg = std::ffi::CString::new(msg).unwrap_or_default();
            unsafe { cb(s_step.as_ptr(), s_msg.as_ptr()) };
        }) as crate::scripting::WorkflowProgressCallback
    });

    rt.block_on(async {
        // Retrieve URL from DB
        match db.get_page_info(id).await {
            Ok(Some(page)) => {
                match crate::video_workflow::scrape_and_save_video(
                    &db_clone,
                    &page.url,
                    &config,
                    on_progress.as_ref(),
                )
                .await
                {
                    Ok(_) => 0,
                    Err(e) => {
                        tracing::error!("FFI: Video scraper failed: {}", e);
                        -1
                    }
                }
            }
            Ok(None) => {
                tracing::error!("FFI: Video {} not found", id);
                -1
            }
            Err(e) => {
                tracing::error!("FFI: DB error: {}", e);
                -1
            }
        }
    })
}
/// Upsert a performer with extended metadata.
/// Returns the performer ID on success, or -1 on failure.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `name` must be a valid C-string. Other string params may be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_upsert_performer(
    ctx: *mut CContext,
    name: *const c_char,
    nation_id: i64,  // Pass -1 for None
    birth_year: i32, // Pass -1 for None
    aliases: *const c_char,
    sex: i32, // Pass -1 for None
) -> i64 {
    check_null!(-1, ctx, name);
    let (rt, db) = get_context!(ctx);

    let name_str = unsafe { CStr::from_ptr(name) }.to_string_lossy();

    let nation_id_opt = if nation_id < 0 { None } else { Some(nation_id) };

    let birth_year_opt = if birth_year < 0 {
        None
    } else {
        Some(birth_year)
    };

    let aliases_str = if aliases.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(aliases) }
                .to_string_lossy()
                .into_owned(),
        )
    };

    let sex_opt = if sex < 0 { None } else { Some(sex) };

    rt.block_on(async {
        match db
            .upsert_performer(
                &name_str,
                nation_id_opt,
                birth_year_opt,
                aliases_str.as_deref(),
                sex_opt,
            )
            .await
        {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("FFI: upsert_performer failed: {}", e);
                -1
            }
        }
    })
}

/// Upsert a page (scene).
/// Returns the page ID on success, or -1 on failure.
///
/// # Safety
/// `ctx` must be a valid context pointer.
/// `url` and `title` must be valid C-strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_upsert_page(
    ctx: *mut CContext,
    url: *const c_char,
    title: *const c_char,
) -> i64 {
    check_null!(-1, ctx, url, title);
    let (rt, db) = get_context!(ctx);

    let url_str = unsafe { CStr::from_ptr(url) }.to_string_lossy();
    let title_str = unsafe { CStr::from_ptr(title) }.to_string_lossy();

    rt.block_on(async {
        match db.upsert_page(&url_str, &title_str).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("FFI: upsert_page failed: {}", e);
                -1
            }
        }
    })
}

/// Link a page to a performer via cast table.
/// Returns 0 on success, -1 on failure.
///
/// # Safety
/// `ctx` must be a valid context pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_link_cast(
    ctx: *mut CContext,
    page_id: i64,
    performer_id: i64,
) -> i32 {
    check_null!(-1, ctx);
    let (rt, db) = get_context!(ctx);

    rt.block_on(async {
        match db.link_cast(page_id, performer_id, 1).await {
            Ok(()) => 0,
            Err(e) => {
                tracing::error!("FFI: link_cast failed: {}", e);
                -1
            }
        }
    })
}

type CProgressCallback = unsafe extern "C" fn(*const c_char, *const c_char);

/// Run model scraper in background.
/// # Safety
/// `ctx` must be a valid context pointer.
/// `url` must be a valid C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_run_model_scraper(
    ctx: *mut CContext,
    url: *const c_char,
    callback: Option<CProgressCallback>,
) -> bool {
    check_null!(false, ctx, url);
    let (rt, db) = get_context!(ctx);
    let url_str = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();

    // Create config from db.config
    let config = crate::model_workflow::WorkflowConfig {
        models_dir: db.config.models_dir.clone(),
        flags_dir: db.config.flags_dir.clone(),
        covers_dir: db.config.covers_dir.clone(),
        thumbs_dir: db.config.thumbs_dir.clone(),
        previews_dir: db.config.previews_dir.clone(),
        scrapers_dir: db.config.scrapers_dir.clone(),
        download_delay_ms: db.config.download_delay_ms,
        ffmpeg_path: db.config.ffmpeg_path.clone(),
        ffprobe_path: db.config.ffprobe_path.clone(),
    };

    // Wrap callback
    let on_progress: Option<crate::model_workflow::WorkflowProgressCallback> = callback
        .map_or_else(
            || None,
            |cb| {
                Some(std::sync::Arc::new(move |step: &str, msg: &str| {
                    let s_step = std::ffi::CString::new(step).unwrap_or_default();
                    let s_msg = std::ffi::CString::new(msg).unwrap_or_default();
                    unsafe { cb(s_step.as_ptr(), s_msg.as_ptr()) };
                })
                    as crate::model_workflow::WorkflowProgressCallback)
            },
        );

    rt.spawn(async move {
        tracing::info!("Starting model scraper for {}", url_str);
        match crate::model_workflow::scrape_and_save_model(
            &db,
            &url_str,
            &config,
            on_progress.as_ref(),
        )
        .await
        {
            Ok(res) => tracing::info!("Model scraper finished: {:?}", res),
            Err(e) => tracing::error!("Model scraper failed: {}", e),
        }
    });

    true
}

/// Run video scraper in background.
/// # Safety
/// `ctx` must be a valid context pointer.
/// `url` must be a valid C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn soromantic_run_video_scraper(
    ctx: *mut CContext,
    url: *const c_char,
    callback: Option<CProgressCallback>,
) -> bool {
    check_null!(false, ctx, url);
    let (rt, db) = get_context!(ctx);
    let url_str = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();

    let config = crate::model_workflow::WorkflowConfig {
        models_dir: db.config.models_dir.clone(),
        flags_dir: db.config.flags_dir.clone(),
        covers_dir: db.config.covers_dir.clone(),
        thumbs_dir: db.config.thumbs_dir.clone(),
        previews_dir: db.config.previews_dir.clone(),
        scrapers_dir: db.config.scrapers_dir.clone(),
        download_delay_ms: db.config.download_delay_ms,
        ffmpeg_path: db.config.ffmpeg_path.clone(),
        ffprobe_path: db.config.ffprobe_path.clone(),
    };

    let on_progress: Option<crate::model_workflow::WorkflowProgressCallback> = callback
        .map_or_else(
            || None,
            |cb| {
                Some(std::sync::Arc::new(move |step: &str, msg: &str| {
                    let s_step = std::ffi::CString::new(step).unwrap_or_default();
                    let s_msg = std::ffi::CString::new(msg).unwrap_or_default();
                    unsafe { cb(s_step.as_ptr(), s_msg.as_ptr()) };
                })
                    as crate::model_workflow::WorkflowProgressCallback)
            },
        );

    rt.spawn(async move {
        tracing::info!("Starting video scraper for {}", url_str);
        match crate::video_workflow::scrape_and_save_video(
            &db,
            &url_str,
            &config,
            on_progress.as_ref(),
        )
        .await
        {
            Ok(res) => tracing::info!("Video scraper finished: {:?}", res),
            Err(e) => tracing::error!("Video scraper failed: {}", e),
        }
    });

    true
}
