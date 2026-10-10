// JNI bridge for Android - reads installed apps and launches them

use jni::objects::{JIntArray, JObject, JValue};
use jni::sys::{jobject, JavaVM as JavaVMPtr};
use jni::JNIEnv;
use log::{error, info};
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use std::sync::Mutex;
use winit::platform::android::activity::AndroidApp;

use crate::android_app_entry::{AppEntry, IconPixels};

/// Global storage for the AndroidApp reference
static ANDROID_APP: OnceCell<Mutex<AndroidApp>> = OnceCell::new();

/// Store the AndroidApp for later use
pub fn set_android_app(app: AndroidApp) {
    let _ = ANDROID_APP.set(Mutex::new(app));
    info!("AndroidApp stored successfully");
}

/// Get the stored AndroidApp
fn get_android_app() -> Option<std::sync::MutexGuard<'static, AndroidApp>> {
    ANDROID_APP.get().map(|m| m.lock().unwrap())
}

/// App-private storage directory (the `Context.getFilesDir()` equivalent).
///
/// Used for `settings.json` and `history.json`: the process working directory
/// is not writable on Android, so nothing may be stored relative to it.
pub fn internal_data_path() -> Option<PathBuf> {
    let path = get_android_app().and_then(|app| app.internal_data_path());
    if path.is_none() {
        error!("Could not resolve the Android internal data path");
    }
    path
}

/// Discover all installed applications using JNI and PackageManager
pub fn discover_apps_jni() -> Vec<AppEntry> {
    let app_guard = match get_android_app() {
        Some(guard) => guard,
        None => {
            error!("AndroidApp not initialized");
            return Vec::new();
        }
    };

    // Get JavaVM pointer from AndroidApp
    let app: &AndroidApp = &*app_guard;
    let vm_ptr = app.vm_as_ptr() as *mut JavaVMPtr;
    if vm_ptr.is_null() {
        error!("JavaVM pointer is null");
        return Vec::new();
    }

    // Create JavaVM from raw pointer
    let vm = match unsafe { jni::JavaVM::from_raw(vm_ptr) } {
        Ok(vm) => vm,
        Err(e) => {
            error!("Failed to create JavaVM: {:?}", e);
            return Vec::new();
        }
    };

    // Get activity object
    let activity_obj = app.activity_as_ptr() as jobject;
    let activity = unsafe { JObject::from_raw(activity_obj) };

    // Attach current thread to JVM
    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            error!("Failed to attach thread: {:?}", e);
            return Vec::new();
        }
    };

    let mut apps = Vec::new();

    // Get PackageManager
    let pm = match env.call_method(
        &activity,
        "getPackageManager",
        "()Landroid/content/pm/PackageManager;",
        &[],
    ) {
        Ok(val) => val.l().unwrap(),
        Err(e) => {
            error!("Failed to get PackageManager: {:?}", e);
            return Vec::new();
        }
    };

    // Get list of installed applications
    let installed_apps = match env.call_method(
        &pm,
        "getInstalledApplications",
        "(I)Ljava/util/List;",
        &[JValue::Int(128)], // GET_META_DATA
    ) {
        Ok(val) => val.l().unwrap(),
        Err(e) => {
            error!("Failed to get installed apps: {:?}", e);
            return Vec::new();
        }
    };

    // Get the size of the list
    let size = match env.call_method(&installed_apps, "size", "()I", &[]) {
        Ok(val) => val.i().unwrap_or(0),
        Err(_) => 0,
    };

    info!("Found {} installed applications", size);

    // Iterate through all installed apps
    for i in 0..size {
        let app_info = match env.call_method(
            &installed_apps,
            "get",
            "(I)Ljava/lang/Object;",
            &[JValue::Int(i)],
        ) {
            Ok(val) => val.l().unwrap(),
            Err(_) => continue,
        };

        if let Some(entry) = process_app_info(&mut env, &pm, &app_info) {
            apps.push(entry);
        }

        let _ = env.delete_local_ref(app_info);
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    info!("Processed {} launchable applications", apps.len());
    apps
}

/// Process a single ApplicationInfo object
fn process_app_info(env: &mut JNIEnv, pm: &JObject, app_info: &JObject) -> Option<AppEntry> {
    // Get package name
    let package_name_field = env
        .get_field(app_info, "packageName", "Ljava/lang/String;")
        .ok()?;
    let package_name_obj = package_name_field.l().ok()?;
    let package_name: String = env.get_string((&package_name_obj).into()).ok()?.into();

    // Check if this app has a launch intent
    let launch_intent = env
        .call_method(
            pm,
            "getLaunchIntentForPackage",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&package_name_obj)],
        )
        .ok()?
        .l()
        .ok()?;

    if launch_intent.is_null() {
        return None;
    }

    // Get app label
    let app_label = env
        .call_method(
            pm,
            "getApplicationLabel",
            "(Landroid/content/pm/ApplicationInfo;)Ljava/lang/CharSequence;",
            &[JValue::Object(app_info)],
        )
        .ok()?
        .l()
        .ok()?;

    let name: String = if !app_label.is_null() {
        let to_string = env
            .call_method(&app_label, "toString", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        let name_str = env.get_string((&to_string).into()).ok()?;
        name_str.into()
    } else {
        package_name.clone()
    };

    let flags = env.get_field(app_info, "flags", "I").ok()?.i().unwrap_or(0);
    let is_system = (flags & 1) != 0;

    let category = get_app_category(env, app_info);
    let tags = build_tags_from_package(&package_name);

    // Use package name as icon identifier (icons will be loaded lazily in UI)
    let icon_id = package_name.clone();

    let _ = env.delete_local_ref(launch_intent);
    let _ = env.delete_local_ref(package_name_obj);

    Some(AppEntry {
        name,
        comment: package_name.clone(),
        exec: package_name,
        icon: icon_id,
        categories: if category.is_empty() {
            vec![if is_system {
                "System".to_string()
            } else {
                "Application".to_string()
            }]
        } else {
            vec![category]
        },
        tags,
        desktop_file: std::path::PathBuf::new(),
        launch_count: 0,
        last_launched: 0,
        is_favorite: false,
    })
}

fn get_app_category(env: &mut JNIEnv, app_info: &JObject) -> String {
    let category_val = match env.get_field(app_info, "category", "I") {
        Ok(val) => val.i().unwrap_or(-1),
        Err(_) => return String::new(),
    };

    match category_val {
        0 => "Game".to_string(),
        1 => "Audio".to_string(),
        2 => "Video".to_string(),
        3 => "Image".to_string(),
        4 => "Social".to_string(),
        5 => "News".to_string(),
        6 => "Maps".to_string(),
        7 => "Productivity".to_string(),
        _ => String::new(),
    }
}

fn build_tags_from_package(package_name: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let parts: Vec<&str> = package_name.split('.').collect();
    for part in parts.iter().rev().take(3) {
        if !["com", "org", "net", "android"].contains(part) && !part.is_empty() {
            tags.push(part.to_string());
        }
    }
    tags.sort();
    tags.dedup();
    tags
}

/// Render an Android application's Drawable into a small RGBA bitmap.
pub fn load_app_icon_jni(package_name: &str) -> Option<IconPixels> {
    let app_guard = get_android_app()?;
    let app: &AndroidApp = &*app_guard;
    let vm_ptr = app.vm_as_ptr() as *mut JavaVMPtr;
    if vm_ptr.is_null() {
        return None;
    }

    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr).ok()? };
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jobject) };
    let mut env = vm.attach_current_thread().ok()?;
    let result = env
        .with_local_frame(32, |env| {
            Ok::<_, jni::errors::Error>(render_app_icon(env, &activity, package_name))
        })
        .ok()
        .flatten();

    // A package can disappear between discovery and rendering. Do not leave a
    // Java exception pending on the UI thread if that happens.
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    result
}

fn render_app_icon(env: &mut JNIEnv, activity: &JObject, package_name: &str) -> Option<IconPixels> {
    const ICON_SIZE: i32 = 96;

    let package_manager = env
        .call_method(
            activity,
            "getPackageManager",
            "()Landroid/content/pm/PackageManager;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;
    let package = env.new_string(package_name).ok()?;
    let drawable = env
        .call_method(
            &package_manager,
            "getApplicationIcon",
            "(Ljava/lang/String;)Landroid/graphics/drawable/Drawable;",
            &[JValue::Object(&package)],
        )
        .ok()?
        .l()
        .ok()?;
    let config = env
        .get_static_field(
            "android/graphics/Bitmap$Config",
            "ARGB_8888",
            "Landroid/graphics/Bitmap$Config;",
        )
        .ok()?
        .l()
        .ok()?;
    let bitmap = env
        .call_static_method(
            "android/graphics/Bitmap",
            "createBitmap",
            "(IILandroid/graphics/Bitmap$Config;)Landroid/graphics/Bitmap;",
            &[
                JValue::Int(ICON_SIZE),
                JValue::Int(ICON_SIZE),
                JValue::Object(&config),
            ],
        )
        .ok()?
        .l()
        .ok()?;
    let canvas = env
        .new_object(
            "android/graphics/Canvas",
            "(Landroid/graphics/Bitmap;)V",
            &[JValue::Object(&bitmap)],
        )
        .ok()?;

    env.call_method(
        &drawable,
        "setBounds",
        "(IIII)V",
        &[
            JValue::Int(0),
            JValue::Int(0),
            JValue::Int(ICON_SIZE),
            JValue::Int(ICON_SIZE),
        ],
    )
    .ok()?;
    env.call_method(
        &drawable,
        "draw",
        "(Landroid/graphics/Canvas;)V",
        &[JValue::Object(&canvas)],
    )
    .ok()?;

    let pixels_array: JIntArray = env.new_int_array(ICON_SIZE * ICON_SIZE).ok()?;
    env.call_method(
        &bitmap,
        "getPixels",
        "([IIIIIII)V",
        &[
            JValue::Object(&pixels_array),
            JValue::Int(0),
            JValue::Int(ICON_SIZE),
            JValue::Int(0),
            JValue::Int(0),
            JValue::Int(ICON_SIZE),
            JValue::Int(ICON_SIZE),
        ],
    )
    .ok()?;

    let mut argb_pixels = vec![0; (ICON_SIZE * ICON_SIZE) as usize];
    env.get_int_array_region(&pixels_array, 0, &mut argb_pixels)
        .ok()?;
    let mut rgba = Vec::with_capacity(argb_pixels.len() * 4);
    for pixel in argb_pixels {
        let pixel = pixel as u32;
        rgba.extend_from_slice(&[
            ((pixel >> 16) & 0xff) as u8,
            ((pixel >> 8) & 0xff) as u8,
            (pixel & 0xff) as u8,
            ((pixel >> 24) & 0xff) as u8,
        ]);
    }

    Some((ICON_SIZE as usize, ICON_SIZE as usize, rgba))
}

/// Launch an app by its package name
pub fn launch_app_jni(package_name: &str) -> Result<(), String> {
    let app_guard = get_android_app().ok_or("AndroidApp not initialized")?;

    let app: &AndroidApp = &*app_guard;
    let vm_ptr = app.vm_as_ptr() as *mut JavaVMPtr;
    if vm_ptr.is_null() {
        return Err("JavaVM pointer is null".to_string());
    }

    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr) }
        .map_err(|e| format!("Failed to create JavaVM: {:?}", e))?;

    let activity_obj = app.activity_as_ptr() as jobject;
    let activity = unsafe { JObject::from_raw(activity_obj) };

    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach thread: {:?}", e))?;

    let pm = env
        .call_method(
            &activity,
            "getPackageManager",
            "()Landroid/content/pm/PackageManager;",
            &[],
        )
        .map_err(|e| format!("Failed to get PackageManager: {:?}", e))?
        .l()
        .map_err(|e| format!("Failed to get PM object: {:?}", e))?;

    let package_jstr = env
        .new_string(package_name)
        .map_err(|e| format!("Failed to create string: {:?}", e))?;

    let launch_intent = env
        .call_method(
            &pm,
            "getLaunchIntentForPackage",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&package_jstr)],
        )
        .map_err(|e| format!("Failed to get launch intent: {:?}", e))?
        .l()
        .map_err(|e| format!("Failed to get intent: {:?}", e))?;

    if launch_intent.is_null() {
        return Err(format!("No launch intent for {}", package_name));
    }

    // Add FLAG_ACTIVITY_NEW_TASK
    let _ = env.call_method(
        &launch_intent,
        "addFlags",
        "(I)Landroid/content/Intent;",
        &[JValue::Int(0x10000000)],
    );

    env.call_method(
        &activity,
        "startActivity",
        "(Landroid/content/Intent;)V",
        &[JValue::Object(&launch_intent)],
    )
    .map_err(|e| format!("Failed to start activity: {:?}", e))?;

    info!("Launched: {}", package_name);
    Ok(())
}
