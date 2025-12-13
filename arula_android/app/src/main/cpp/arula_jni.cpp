#include <jni.h>
#include <android/log.h>
#include <string>
#include <mutex>
#include <thread>
#include <atomic>

#define LOG_TAG "ArulaJNI"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

extern "C" {
    // Rust functions that will be implemented in arula_core
    bool rust_initialize(const char* config_json);
    void rust_send_message(const char* message);
    void rust_set_config(const char* config_json);
    const char* rust_get_config();
    void rust_cleanup();
    void rust_set_java_callback(JNIEnv* env, jobject callback);
}

static JavaVM* g_jvm = nullptr;
static jobject g_callback = nullptr;
static std::mutex g_callback_mutex;

// JNI_OnLoad - Called when library is loaded
JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM* vm, void* reserved) {
    LOGI("JNI_OnLoad called");
    g_jvm = vm;
    return JNI_VERSION_1_8;
}

// Helper to get JNIEnv
JNIEnv* getJNIEnv() {
    JNIEnv* env = nullptr;
    if (g_jvm->GetEnv(reinterpret_cast<void**>(&env), JNI_VERSION_1_8) != JNI_OK) {
        LOGE("Failed to get JNIEnv");
        return nullptr;
    }
    return env;
}

// Helper to call Java callback methods
void callJavaMethod(const char* methodName, const char* signature, jstring value) {
    std::lock_guard<std::mutex> lock(g_callback_mutex);
    if (!g_callback) return;

    JNIEnv* env = getJNIEnv();
    if (!env) return;

    jclass callbackClass = env->GetObjectClass(g_callback);
    if (!callbackClass) {
        LOGE("Failed to get callback class");
        return;
    }

    jmethodID method = env->GetMethodID(callbackClass, methodName, signature);
    if (!method) {
        LOGE("Failed to get method ID for %s", methodName);
        env->DeleteLocalRef(callbackClass);
        return;
    }

    env->CallVoidMethod(g_callback, method, value);

    if (env->ExceptionCheck()) {
        LOGE("Exception calling %s", methodName);
        env->ExceptionDescribe();
        env->ExceptionClear();
    }

    env->DeleteLocalRef(callbackClass);
}

// Rust callback functions
extern "C" void rust_on_message(const char* message) {
    JNIEnv* env = getJNIEnv();
    if (!env) return;
    jstring jMessage = env->NewStringUTF(message);
    callJavaMethod("onMessage", "(Ljava/lang/String;)V", jMessage);
    env->DeleteLocalRef(jMessage);
}

extern "C" void rust_on_stream_chunk(const char* chunk) {
    JNIEnv* env = getJNIEnv();
    if (!env) return;
    jstring jChunk = env->NewStringUTF(chunk);
    callJavaMethod("onStreamChunk", "(Ljava/lang/String;)V", jChunk);
    env->DeleteLocalRef(jChunk);
}

extern "C" void rust_on_tool_start(const char* tool_name, const char* tool_id) {
    JNIEnv* env = getJNIEnv();
    if (!env) return;
    jstring jToolName = env->NewStringUTF(tool_name);
    jstring jToolId = env->NewStringUTF(tool_id);

    std::lock_guard<std::mutex> lock(g_callback_mutex);
    if (!g_callback) {
        env->DeleteLocalRef(jToolName);
        env->DeleteLocalRef(jToolId);
        return;
    }

    jclass callbackClass = env->GetObjectClass(g_callback);
    jmethodID method = env->GetMethodID(callbackClass, "onToolStart",
        "(Ljava/lang/String;Ljava/lang/String;)V");

    if (method) {
        env->CallVoidMethod(g_callback, method, jToolName, jToolId);
        if (env->ExceptionCheck()) {
            env->ExceptionDescribe();
            env->ExceptionClear();
        }
    }

    env->DeleteLocalRef(callbackClass);
    env->DeleteLocalRef(jToolName);
    env->DeleteLocalRef(jToolId);
}

extern "C" void rust_on_tool_complete(const char* tool_id, const char* result) {
    JNIEnv* env = getJNIEnv();
    if (!env) return;
    jstring jToolId = env->NewStringUTF(tool_id);
    jstring jResult = env->NewStringUTF(result);

    std::lock_guard<std::mutex> lock(g_callback_mutex);
    if (!g_callback) {
        env->DeleteLocalRef(jToolId);
        env->DeleteLocalRef(jResult);
        return;
    }

    jclass callbackClass = env->GetObjectClass(g_callback);
    jmethodID method = env->GetMethodID(callbackClass, "onToolComplete",
        "(Ljava/lang/String;Ljava/lang/String;)V");

    if (method) {
        env->CallVoidMethod(g_callback, method, jToolId, jResult);
        if (env->ExceptionCheck()) {
            env->ExceptionDescribe();
            env->ExceptionClear();
        }
    }

    env->DeleteLocalRef(callbackClass);
    env->DeleteLocalRef(jToolId);
    env->DeleteLocalRef(jResult);
}

extern "C" void rust_on_error(const char* error) {
    JNIEnv* env = getJNIEnv();
    if (!env) return;
    jstring jError = env->NewStringUTF(error);
    callJavaMethod("onError", "(Ljava/lang/String;)V", jError);
    env->DeleteLocalRef(jError);
}

// JNI implementations

JNIEXPORT jboolean JNICALL
Java_com_arula_terminal_ArulaNative_initialize(JNIEnv* env, jclass clazz, jstring config_json) {
    const char* config = env->GetStringUTFChars(config_json, nullptr);
    bool result = rust_initialize(config);
    env->ReleaseStringUTFChars(config_json, config);
    return static_cast<jboolean>(result);
}

JNIEXPORT void JNICALL
Java_com_arula_terminal_ArulaNative_sendMessage(JNIEnv* env, jclass clazz, jstring message) {
    const char* msg = env->GetStringUTFChars(message, nullptr);
    rust_send_message(msg);
    env->ReleaseStringUTFChars(message, msg);
}

JNIEXPORT void JNICALL
Java_com_arula_terminal_ArulaNative_setConfig(JNIEnv* env, jclass clazz, jstring config_json) {
    const char* config = env->GetStringUTFChars(config_json, nullptr);
    rust_set_config(config);
    env->ReleaseStringUTFChars(config_json, config);
}

JNIEXPORT jstring JNICALL
Java_com_arula_terminal_ArulaNative_getConfig(JNIEnv* env, jclass clazz) {
    const char* config = rust_get_config();
    jstring result = env->NewStringUTF(config);
    return result;
}

JNIEXPORT void JNICALL
Java_com_arula_terminal_ArulaNative_cleanup(JNIEnv* env, jclass clazz) {
    std::lock_guard<std::mutex> lock(g_callback_mutex);
    if (g_callback) {
        env->DeleteGlobalRef(g_callback);
        g_callback = nullptr;
    }
    rust_cleanup();
}

JNIEXPORT void JNICALL
Java_com_arula_terminal_ArulaNative_setCallback(JNIEnv* env, jclass clazz, jobject callback) {
    std::lock_guard<std::mutex> lock(g_callback_mutex);
    if (g_callback) {
        env->DeleteGlobalRef(g_callback);
    }
    g_callback = callback ? env->NewGlobalRef(callback) : nullptr;
    rust_set_java_callback(env, callback);
}