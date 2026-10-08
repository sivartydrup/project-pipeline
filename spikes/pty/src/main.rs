use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn shell_command() -> CommandBuilder {
    if cfg!(windows) && std::env::var_os("PIPELINE_USE_POWERSHELL").is_some() {
        let mut command = CommandBuilder::new("powershell.exe");
        command.arg("-NoLogo");
        command.arg("-NoProfile");
        command
    } else if cfg!(windows) {
        let mut command = CommandBuilder::new("cmd.exe");
        command.arg("/Q");
        command
    } else {
        CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned()))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pty = native_pty_system();
    let pair = pty.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let mut child = pair.slave.spawn_command(shell_command())?;
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;

    let (sender, receiver) = mpsc::sync_channel::<Vec<u8>>(64);
    let reader_handle = thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    if sender.send(buffer[..count].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    pair.master.resize(PtySize {
        rows: 30,
        cols: 100,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let size = pair.master.get_size()?;
    if size.rows != 30 || size.cols != 100 {
        return Err(format!("resize failed: {size:?}").into());
    }

    // Windows ConPTY may query the terminal cursor position before presenting
    // the shell prompt. A terminal host must answer this request.
    if cfg!(windows) {
        writer.write_all(b"\x1b[1;1R")?;
        writer.flush()?;
    }

    let marker = "PIPELINE_PTY_OK";
    let use_powershell = cfg!(windows) && std::env::var_os("PIPELINE_USE_POWERSHELL").is_some();
    let command = if use_powershell {
        format!(
            "Write-Output (\"$([char]27)[31mRED$([char]27)[0m λ\"); Write-Output ('PIPELINE' + '_PTY_OK')\r\n"
        )
    } else if cfg!(windows) {
        "echo PIPELINE^_PTY_OK\r\n".to_owned()
    } else {
        "echo PIPELINE'_PTY_OK'\r\n".to_owned()
    };
    writer.write_all(command.as_bytes())?;
    writer.flush()?;

    let mut parser = vt100::Parser::new(30, 100, 1000);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut found = false;
    let mut raw = Vec::new();
    while Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(bytes) => {
                raw.extend_from_slice(&bytes);
                parser.process(&bytes);
                if parser.screen().contents().contains(marker) {
                    found = true;
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    if !found {
        child.kill()?;
        return Err(format!(
            "shell marker did not appear; captured output: {}",
            String::from_utf8_lossy(&raw)
        )
        .into());
    }
    if use_powershell && !parser.screen().contents().contains("RED λ") {
        child.kill()?;
        return Err(format!(
            "PowerShell ANSI/Unicode output missing; captured output: {}",
            String::from_utf8_lossy(&raw)
        )
        .into());
    }

    writer.write_all(b"exit\r\n")?;
    writer.flush()?;
    let exit_deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait()?.is_none() && Instant::now() < exit_deadline {
        thread::sleep(Duration::from_millis(100));
    }
    if child.try_wait()?.is_none() {
        child.kill()?;
        return Err("shell did not exit after exit command".into());
    }
    drop(writer);
    drop(pair);
    drop(receiver);
    let _ = reader_handle.join();

    println!("PTY OK: shell output, resize to 100x30, ANSI parser, and clean exit");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_ansi_color_and_unicode() {
        let mut parser = vt100::Parser::new(4, 20, 10);
        parser.process("\u{1b}[31mRED\u{1b}[0m λ".as_bytes());
        assert!(parser.screen().contents().contains("RED λ"));
        assert_eq!(
            parser.screen().cell(0, 0).unwrap().fgcolor(),
            vt100::Color::Idx(1)
        );
    }

    #[test]
    fn retains_scrollback() {
        let mut parser = vt100::Parser::new(4, 20, 10);
        for index in 0..12 {
            parser.process(format!("line-{index}\r\n").as_bytes());
        }
        parser.screen_mut().set_scrollback(10);
        assert!(parser.screen().contents().contains("line-2"));
    }
}
