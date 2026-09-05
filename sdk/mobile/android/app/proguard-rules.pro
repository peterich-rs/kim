# Flutter 3.47 enables R8 minify for release by default.
# Keep JNI / flutter_rust_bridge native entry points.
-keepclasseswithmembernames class * {
    native <methods>;
}
-keep class io.flutter.embedding.** { *; }
-keep class io.flutter.plugin.** { *; }
-keep class io.flutter.util.** { *; }
-keep class io.flutter.view.** { *; }
-keep class io.flutter.embedding.engine.plugins.FlutterPlugin { *; }

# flutter_rust_bridge generated Dart talks to the native asset via JNI/FFI.
-keep class com.kim.kim_mobile.** { *; }
-keep class com.kim.kim_media_picker.** { *; }

# Flutter embedding references Play Core deferred components. This app
# does not ship Play Feature Delivery; R8 must not fail on those classes.
-dontwarn com.google.android.play.core.splitcompat.SplitCompatApplication
-dontwarn com.google.android.play.core.splitinstall.**
-dontwarn com.google.android.play.core.tasks.**
