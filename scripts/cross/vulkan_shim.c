/*
 * ggml's Vulkan backend takes vkGetInstanceProcAddr (and two functions it
 * calls directly) from the Vulkan loader, and resolves everything else
 * through it. Linking vulkan-1.dll directly would stop Crosstalk from
 * starting on a PC without a Vulkan driver; this shim (built as libvulkan.a by
 * cross-build-windows-gnu.sh) loads it at run time from System32 instead.
 * Without it, the only function it returns is a vkEnumerateInstanceVersion
 * that fails: ggml then reports no GPU and Whisper runs on the CPU. The direct
 * calls are only made on a GPU found through the loader.
 */
#include <string.h>
#include <windows.h>

typedef void (*vk_void_fn)(void);
typedef vk_void_fn (*vk_get_proc_fn)(void *instance, const char *name);

static HMODULE loader;
static vk_get_proc_fn real_get_proc;
static volatile LONG tried;

static HMODULE vulkan_dll(void) {
    if (InterlockedCompareExchange(&tried, 1, 0) == 0) {
        loader = LoadLibraryExW(L"vulkan-1.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
        if (loader) {
            real_get_proc = (vk_get_proc_fn)(void *)GetProcAddress(loader, "vkGetInstanceProcAddr");
        }
        MemoryBarrier();
    }
    return loader;
}

/* VK_ERROR_INITIALIZATION_FAILED: Vulkan-Hpp throws, ggml catches it. */
static int vk_unavailable(unsigned int *version) {
    (void)version;
    return -3;
}

vk_void_fn vkGetInstanceProcAddr(void *instance, const char *name) {
    vulkan_dll();
    if (real_get_proc) {
        return real_get_proc(instance, name);
    }
    if (name && strcmp(name, "vkEnumerateInstanceVersion") == 0) {
        return (vk_void_fn)vk_unavailable;
    }
    return NULL;
}

/* Core functions exported by vulkan-1.dll (handles are pointers on x64). */
typedef void (*copy_buffer_fn)(void *, void *, void *, unsigned int, const void *);
typedef void (*features2_fn)(void *, void *);

void vkCmdCopyBuffer(void *cmd, void *src, void *dst, unsigned int count, const void *regions) {
    static copy_buffer_fn fn;
    if (!fn) {
        fn = (copy_buffer_fn)(void *)GetProcAddress(vulkan_dll(), "vkCmdCopyBuffer");
    }
    fn(cmd, src, dst, count, regions);
}

void vkGetPhysicalDeviceFeatures2(void *device, void *features) {
    static features2_fn fn;
    if (!fn) {
        fn = (features2_fn)(void *)GetProcAddress(vulkan_dll(), "vkGetPhysicalDeviceFeatures2");
    }
    fn(device, features);
}
