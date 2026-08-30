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
