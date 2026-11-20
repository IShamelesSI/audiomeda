//! Windows plumbing: processes, devices, audio sessions, per-app routing.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::c_void;

use wasapi::{DeviceEnumerator, Direction, SessionState};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::{
    EDataFlow, ERole, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume,
    MMDeviceEnumerator, eConsole, eMultimedia, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows_core::Interface;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::WinRT::RoGetActivationFactory;
use windows_core::{HRESULT, HSTRING, IUnknown, IUnknown_Vtbl, interface};

pub type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct Proc {
    pub pid: u32,
    pub parent: u32,
    pub exe: String, // lowercase
}

pub fn processes() -> Res<Vec<Proc>> {
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            out.push(Proc {
                pid: e.th32ProcessID,
                parent: e.th32ParentProcessID,
                exe: String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase(),
            });
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        CloseHandle(snap)?;
    }
    Ok(out)
}

/// Root of the exe's process tree (browsers are many processes; loopback must target the root).
pub fn root_pid(procs: &[Proc], exe: &str) -> Option<u32> {
    let mine: Vec<&Proc> = procs.iter().filter(|p| p.exe == exe).collect();
    mine.iter()
        .find(|p| !mine.iter().any(|q| q.pid == p.parent))
        .or(mine.first())
        .map(|p| p.pid)
}

/// id -> friendly name of active devices.
pub fn devices(dir: Direction) -> Res<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for d in &DeviceEnumerator::new()?.get_device_collection(dir)? {
        let d = d?;
        out.insert(d.get_id()?, d.get_friendlyname()?);
    }
    Ok(out)
}

pub fn default_render() -> Res<(String, String)> {
    let d = DeviceEnumerator::new()?.get_default_device(Direction::Render)?;
    Ok((d.get_id()?, d.get_friendlyname()?))
}

/// Pids with an active audio session on any render device except `skip`.
pub fn playing_pids(skip: &[&str]) -> Res<Vec<u32>> {
    let mut out = Vec::new();
    for d in &DeviceEnumerator::new()?.get_device_collection(Direction::Render)? {
        let d = d?;
        if skip.contains(&d.get_id()?.as_str()) {
            continue;
        }
        let sessions = d.get_iaudiosessionmanager()?.get_audiosessionenumerator()?;
        for i in 0..sessions.get_count()? {
            let s = sessions.get_session(i)?;
            if s.get_state()? == SessionState::Active {
                out.push(s.get_process_id().unwrap_or(0));
            }
        }
    }
    Ok(out)
}

/// Undocumented `Windows.Media.Internal.AudioPolicyConfig` factory (Win11 IID, layout from EarTrumpet).
/// Declared on IUnknown, so the first 3 placeholders stand in for IInspectable's methods.
#[allow(non_snake_case)]
mod policy {
    use super::*;
    #[interface("ab3d4648-e242-459f-b02f-541c70306324")]
    pub unsafe trait IAudioPolicyConfigFactory: IUnknown {
        fn _i0(&self) -> HRESULT;
        fn _i1(&self) -> HRESULT;
        fn _i2(&self) -> HRESULT;
        fn _u0(&self) -> HRESULT;
        fn _u1(&self) -> HRESULT;
        fn _u2(&self) -> HRESULT;
        fn _u3(&self) -> HRESULT;
        fn _u4(&self) -> HRESULT;
        fn _u5(&self) -> HRESULT;
        fn _u6(&self) -> HRESULT;
        fn _u7(&self) -> HRESULT;
        fn _u8(&self) -> HRESULT;
        fn _u9(&self) -> HRESULT;
        fn _u10(&self) -> HRESULT;
        fn _u11(&self) -> HRESULT;
        fn _u12(&self) -> HRESULT;
        fn _u13(&self) -> HRESULT;
        fn _u14(&self) -> HRESULT;
        fn _u15(&self) -> HRESULT;
        fn _u16(&self) -> HRESULT;
        fn _u17(&self) -> HRESULT;
        fn _u18(&self) -> HRESULT;
        pub fn SetPersistedDefaultAudioEndpoint(
            &self,
            pid: u32,
            flow: EDataFlow,
            role: ERole,
            device_id: *mut c_void, // HSTRING handle, null = system default
        ) -> HRESULT;
    }
}

/// Route every process of `exe` to a render device, or back to the system default (None).
/// Sandboxed child processes reject this (E_INVALIDARG); fine as long as one accepts.
pub fn route(procs: &[Proc], exe: &str, device_id: Option<&str>) -> Res<()> {
    let factory: policy::IAudioPolicyConfigFactory =
        unsafe { RoGetActivationFactory(&HSTRING::from("Windows.Media.Internal.AudioPolicyConfig"))? };
    let id = device_id.map(|d| {
        HSTRING::from(format!(r"\\?\SWD#MMDEVAPI#{d}#{{e6327cad-dcec-4949-ae8a-991e976a79d2}}"))
    });
    // the ABI wants the HSTRING handle itself, not the text pointer from as_ptr()
    let raw: *mut c_void = id.as_ref().map_or(std::ptr::null_mut(), |h| unsafe { std::mem::transmute_copy(h) });
    let mut accepted = false;
    for p in procs.iter().filter(|p| p.exe == exe) {
        accepted |= [eConsole, eMultimedia]
            .map(|role| unsafe { factory.SetPersistedDefaultAudioEndpoint(p.pid, eRender, role, raw) })
            .iter()
            .all(|h| h.is_ok());
    }
    if accepted { Ok(()) } else { Err(format!("no process of {exe} accepted the route").into()) }
}

/// Loopback capture hears the app *after* its Windows session volume, which Windows remembers
/// per device. Keep managed apps at 100% / unmuted on the sink so only the mixer's volumes count.
/// Returns how many sessions were changed.
pub fn unity_on(device_id: &str, pids: &HashSet<u32>) -> Res<usize> {
    let mut changed = 0;
    unsafe {
        let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let mgr: IAudioSessionManager2 = en.GetDevice(&HSTRING::from(device_id))?.Activate(CLSCTX_ALL, None)?;
        let sessions = mgr.GetSessionEnumerator()?;
        for i in 0..sessions.GetCount()? {
            let s: IAudioSessionControl2 = sessions.GetSession(i)?.cast()?;
            if !pids.contains(&s.GetProcessId().unwrap_or(0)) {
                continue;
            }
            let v: ISimpleAudioVolume = s.cast()?;
            if v.GetMasterVolume()? < 1.0 || v.GetMute()?.as_bool() {
                v.SetMasterVolume(1.0, std::ptr::null())?;
                v.SetMute(false, std::ptr::null())?;
                changed += 1;
            }
        }
    }
    Ok(changed)
}

pub fn exe_by_pid(procs: &[Proc]) -> HashMap<u32, &str> {
    procs.iter().map(|p| (p.pid, p.exe.as_str())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_the_process_whose_parent_is_another_exe() {
        let p = |pid, parent, exe: &str| Proc { pid, parent, exe: exe.into() };
        let procs = vec![p(5, 1, "explorer.exe"), p(20, 10, "msedge.exe"), p(10, 5, "msedge.exe"), p(30, 10, "msedge.exe")];
        assert_eq!(root_pid(&procs, "msedge.exe"), Some(10));
        assert_eq!(root_pid(&procs, "nope.exe"), None);
    }
}
