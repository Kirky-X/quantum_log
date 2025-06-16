//! 背景信息获取模块
//!
//! 提供获取系统背景信息的函数，包括进程ID、线程ID、用户名、主机名等。

use std::ffi::OsString;

#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;

/// 系统背景信息结构体
///
/// 包含进程ID、线程ID、用户名、主机名等系统信息
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackgroundInfo {
    /// 进程ID
    pub pid: u32,
    /// 线程ID
    pub tid: String,
    /// 用户名
    pub username: String,
    /// 主机名
    pub hostname: String,
}

impl BackgroundInfo {
    /// 创建新的背景信息实例
    ///
    /// # Returns
    ///
    /// 返回包含当前系统信息的BackgroundInfo实例
    ///
    /// # Examples
    ///
    /// ```
    /// use quantum_log::utils::background_info::BackgroundInfo;
    ///
    /// let info = BackgroundInfo::new();
    /// println!("Process ID: {}", info.pid);
    /// ```
    pub fn new() -> Self {
        Self {
            pid: get_pid(),
            tid: get_tid(),
            username: get_username(),
            hostname: get_hostname(),
        }
    }

    /// 获取进程ID
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// 获取线程ID
    pub fn tid(&self) -> &str {
        &self.tid
    }

    /// 获取用户名
    pub fn username(&self) -> &str {
        &self.username
    }

    /// 获取主机名
    pub fn hostname(&self) -> &str {
        &self.hostname
    }
}

/// 获取当前进程ID
///
/// # Returns
///
/// 返回当前进程的ID
///
/// # Examples
///
/// ```
/// use quantum_log::utils::background_info::get_pid;
///
/// let pid = get_pid();
/// println!("Current process ID: {}", pid);
/// ```
pub fn get_pid() -> u32 {
    std::process::id()
}

/// 获取当前线程ID
///
/// # Returns
///
/// 返回当前线程的ID字符串表示
///
/// # Examples
///
/// ```
/// use quantum_log::utils::background_info::get_tid;
///
/// let tid = get_tid();
/// println!("Current thread ID: {}", tid);
/// ```
pub fn get_tid() -> String {
    // 在Windows上获取线程ID
    #[cfg(windows)]
    {
        unsafe {
            let tid = winapi::um::processthreadsapi::GetCurrentThreadId();
            tid.to_string()
        }
    }

    // 在Unix系统上获取线程ID
    #[cfg(unix)]
    {
        // 使用gettid系统调用
        unsafe {
            let tid = libc::syscall(libc::SYS_gettid);
            tid.to_string()
        }
    }

    // 其他平台的fallback
    #[cfg(not(any(windows, unix)))]
    {
        use std::thread;
        format!("{:?}", thread::current().id())
    }
}

/// 获取当前用户名
///
/// # Returns
///
/// 返回当前用户的用户名，如果获取失败则返回"unknown"
///
/// # Examples
///
/// ```
/// use quantum_log::utils::background_info::get_username;
///
/// let username = get_username();
/// println!("Current username: {}", username);
/// ```
pub fn get_username() -> String {
    #[cfg(windows)]
    {
        get_username_windows().unwrap_or_else(|| "unknown".to_string())
    }

    #[cfg(unix)]
    {
        get_username_unix().unwrap_or_else(|| "unknown".to_string())
    }

    #[cfg(not(any(windows, unix)))]
    {
        "unknown".to_string()
    }
}

/// 获取主机名
///
/// # Returns
///
/// 返回当前设备的主机名，如果获取失败则返回"unknown"
///
/// # Examples
///
/// ```
/// use quantum_log::utils::background_info::get_hostname;
///
/// let hostname = get_hostname();
/// println!("Current hostname: {}", hostname);
/// ```
pub fn get_hostname() -> String {
    hostname::get()
        .map(|name| name.to_string_lossy().into_owned())
        .ok()
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(windows)]
fn get_username_windows() -> Option<String> {
    use winapi::shared::minwindef::DWORD;
    use winapi::um::winbase::GetUserNameW;

    let mut buffer = vec![0u16; 256];
    let mut size = buffer.len() as DWORD;

    unsafe {
        if GetUserNameW(buffer.as_mut_ptr(), &mut size) != 0 {
            buffer.truncate((size - 1) as usize); // 移除null终止符
            let os_string = OsString::from_wide(&buffer);
            os_string.into_string().ok()
        } else {
            None
        }
    }
}

#[cfg(unix)]
fn get_username_unix() -> Option<String> {
    use std::env;

    // 首先尝试环境变量
    if let Ok(user) = env::var("USER") {
        return Some(user);
    }

    if let Ok(user) = env::var("USERNAME") {
        return Some(user);
    }

    // 使用getpwuid获取用户信息
    unsafe {
        let uid = libc::getuid();
        let passwd = libc::getpwuid(uid);
        if !passwd.is_null() {
            let name_ptr = (*passwd).pw_name;
            if !name_ptr.is_null() {
                let c_str = std::ffi::CStr::from_ptr(name_ptr);
                return c_str.to_string_lossy().into_owned().into();
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_pid() {
        let pid = get_pid();
        assert!(pid > 0);
    }

    #[test]
    fn test_get_tid() {
        let tid = get_tid();
        assert!(!tid.is_empty());
    }

    #[test]
    fn test_get_username() {
        let username = get_username();
        assert!(!username.is_empty());
        assert_ne!(username, "unknown");
    }

    #[test]
    fn test_get_hostname() {
        let hostname = get_hostname();
        assert!(!hostname.is_empty());
        assert_ne!(hostname, "unknown");
    }

    #[test]
    fn test_multiple_calls_consistency() {
        // PID应该在同一进程中保持一致
        let pid1 = get_pid();
        let pid2 = get_pid();
        assert_eq!(pid1, pid2);

        // 用户名应该保持一致
        let username1 = get_username();
        let username2 = get_username();
        assert_eq!(username1, username2);

        // 主机名应该保持一致
        let hostname1 = get_hostname();
        let hostname2 = get_hostname();
        assert_eq!(hostname1, hostname2);
    }
}
