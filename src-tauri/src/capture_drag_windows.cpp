#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <ole2.h>
#include <shlobj.h>

class CaptureFileDropSource final : public IDropSource {
public:
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void **out) override {
        if (!out) return E_POINTER;
        *out = nullptr;
        if (iid != IID_IUnknown && iid != IID_IDropSource) return E_NOINTERFACE;
        *out = static_cast<IDropSource *>(this);
        AddRef();
        return S_OK;
    }

    ULONG STDMETHODCALLTYPE AddRef() override { return ++references; }

    ULONG STDMETHODCALLTYPE Release() override {
        ULONG remaining = --references;
        if (!remaining) delete this;
        return remaining;
    }

    HRESULT STDMETHODCALLTYPE QueryContinueDrag(BOOL escaped, DWORD keys) override {
        if (escaped) return DRAGDROP_S_CANCEL;
        if (!(keys & MK_LBUTTON)) return DRAGDROP_S_DROP;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE GiveFeedback(DWORD) override {
        return DRAGDROP_S_USEDEFAULTCURSORS;
    }

private:
    ULONG references = 1;
};

extern "C" int snapshoot_drag_file(const wchar_t *path) {
    if (!path || !(GetAsyncKeyState(VK_LBUTTON) & 0x8000)) return 0;
    HRESULT initialized = OleInitialize(nullptr);
    if (FAILED(initialized)) return 0;

    PIDLIST_ABSOLUTE item = nullptr;
    PCUITEMID_CHILD child = nullptr;
    IShellFolder *folder = nullptr;
    IDataObject *data = nullptr;
    HRESULT result = SHParseDisplayName(path, nullptr, &item, 0, nullptr);
    if (SUCCEEDED(result)) result = SHBindToParent(item, IID_PPV_ARGS(&folder), &child);
    if (SUCCEEDED(result)) result = folder->GetUIObjectOf(nullptr, 1, &child,
        IID_IDataObject, nullptr, reinterpret_cast<void **>(&data));
    if (SUCCEEDED(result)) {
        CaptureFileDropSource *source = new CaptureFileDropSource();
        DWORD effect = DROPEFFECT_NONE;
        result = DoDragDrop(data, source, DROPEFFECT_COPY, &effect);
        source->Release();
    }
    if (data) data->Release();
    if (folder) folder->Release();
    CoTaskMemFree(item);
    OleUninitialize();
    return SUCCEEDED(result);
}
