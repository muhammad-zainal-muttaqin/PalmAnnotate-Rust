# Orbbec exposes JNI entry points whose Java names must remain stable.
-keep class com.orbbec.** { *; }

# Tauri's native runtime (tao/wry) calls into the GENERATED app classes by exact
# JVM signature via JNI reflection — e.g. MainActivity.getId()I, RustWebView,
# RustWebChromeClient, Logger, the WryActivity base, and the native-plugin bridge.
# R8 must not rename, remove, or inline any of them, or the app SIGABRTs on launch
# with NoSuchMethodError before the first frame.
-keep class dev.sawitulm.** { *; }
-keepclassmembers class dev.sawitulm.** { *; }

# Tauri runtime + plugins (geolocation, palm-native) are resolved reflectively by
# fully-qualified class name and dispatched through @Command-annotated methods.
-keep class app.tauri.** { *; }
-keepattributes *Annotation*
-keepclasseswithmembers class * {
    @app.tauri.annotation.Command <methods>;
}

# Never strip the *names* of native methods anywhere (JNI binds by name).
-keepclasseswithmembernames class * {
    native <methods>;
}

# androidx Activity result/JNI callbacks invoked from native code.
-keepclassmembers class * extends android.app.Activity {
    public <init>(...);
    public *;
}
