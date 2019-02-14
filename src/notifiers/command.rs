use std::process::Command as StdCommand;

use futures::future::{ok, Either};
use log::{info, warn};
use tokio::prelude::*;
use tokio_process::CommandExt;

use crate::Notifier;

pub struct Command {
    pub cmd: Vec<String>,
}

impl Notifier for Command {
    fn notify(&self, name: String, early: Option<u64>) {
        info!("running notify command: cmd={}", self.cmd.join(" "));

        let proc = StdCommand::new(&self.cmd[0])
            .args(self.cmd[1..].into_iter())
            .env("CONDEMN_NAME", name)
            .env("CONDEMN_EARLY", format!("{}", early.unwrap_or(0)))
            .spawn_async();

        tokio::spawn(match proc {
            Ok(f) => Either::A(
                f.map(|status| info!("command exited with status {}", status))
                    .map_err(|e| warn!("failed to wait for exit: {}", e)),
            ),
            Err(e) => {
                warn!("failed to spawn command; {}", e);
                Either::B(ok(()))
            }
        });
    }
}
