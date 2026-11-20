use crate::bridge::{Bridge, Operation, Reply};
use std::{ffi::c_void, io, ptr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    sync::Semaphore,
    time::timeout,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

const MAX_REQUEST: u64 = 65_536;

/// プロセストークンのユーザー SID。パイプ名と明示的 DACL に同じ値を使う。
fn current_user_sid() -> io::Result<String> {
    unsafe {
        let mut token: HANDLE = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut size = 0;
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut size);
        let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        );
        CloseHandle(token);
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text = ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut length = 0;
        while *text.add(length) != 0 {
            length += 1;
        }
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
        LocalFree(text.cast());
        Ok(sid)
    }
}

fn create_server(name: &str, sid: &str, first: bool) -> io::Result<NamedPipeServer> {
    // 既定の DACL は使わず、同じユーザーだけを許可する。
    let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .max_instances(16)
            .create_with_security_attributes_raw(
                name,
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast::<c_void>(),
            );
        LocalFree(descriptor.cast());
        result
    }
}

pub fn start(bridge: Arc<Bridge>) -> io::Result<()> {
    let sid = current_user_sid()?;
    let name = format!(r"\\.\pipe\paruma.tasktree2026.{sid}");
    let server = tauri::async_runtime::block_on(async { create_server(&name, &sid, true) })?;
    tauri::async_runtime::spawn(async move {
        let limit = Arc::new(Semaphore::new(8));
        let mut server = server;
        loop {
            if let Err(error) = server.connect().await {
                eprintln!("パイプ接続に失敗: {error}");
                break;
            }
            // 接続中のハンドルを保持したまま次を作り、名前の乗っ取り可能な空白を作らない。
            let next = match create_server(&name, &sid, false) {
                Ok(next) => next,
                Err(error) => {
                    eprintln!("パイプを再作成できません: {error}");
                    break;
                }
            };
            let connected = server;
            server = next;
            let Ok(permit) = limit.clone().try_acquire_owned() else {
                drop(connected);
                continue;
            };
            let bridge = bridge.clone();
            tauri::async_runtime::spawn(async move {
                let _permit = permit;
                let mut reader = BufReader::new(connected);
                let reply = match timeout(Duration::from_secs(5), read_request(&mut reader)).await {
                    Ok(Ok(request)) => bridge.dispatch(request).await,
                    Ok(Err(error)) => Reply::error(error.to_string()),
                    Err(_) => Reply::error("要求の受信がタイムアウトしました"),
                };
                let mut bytes = serde_json::to_vec(&reply).unwrap();
                bytes.push(b'\n');
                let _ = timeout(Duration::from_secs(5), reader.get_mut().write_all(&bytes)).await;
            });
        }
    });
    Ok(())
}

async fn read_request(reader: &mut BufReader<NamedPipeServer>) -> io::Result<Operation> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_REQUEST + 1)
        .read_until(b'\n', &mut bytes)
        .await?;
    if bytes.len() as u64 > MAX_REQUEST || bytes.last() != Some(&b'\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "要求は改行終端の UTF-8 JSON、64 KiB 以下にしてください",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}
