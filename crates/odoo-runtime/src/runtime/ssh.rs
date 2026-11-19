//! The app's own SSH key for GitHub, so people never touch SSH: made once in the
//! runtime folder (never in `~/.ssh`), added to their GitHub account with their
//! sign-in, and used only by the app's own Git commands.

use super::*;

impl Runtime {
    fn ssh_dir(&self) -> PathBuf {
        self.root.join("ssh")
    }

    /// The private key; its public half is next to it (`.pub`).
    pub fn ssh_key_path(&self) -> PathBuf {
        self.ssh_dir().join("id_ed25519")
    }

    /// Make the key unless it exists; returns the public key ("ssh-ed25519 ...").
    pub fn ensure_ssh_key(&self) -> Result<String> {
        let key = self.ssh_key_path();
        let public = key.with_extension("pub");
        if !(key.is_file() && public.is_file()) {
            fs::create_dir_all(self.ssh_dir())?;
            let _ = fs::remove_file(&key);
            let _ = fs::remove_file(&public);
            let path = key.display().to_string();
            self.run_to_end(
                &self.tools.ssh_keygen,
                &[
                    "-t",
                    "ed25519",
                    "-N",
                    "",
                    "-C",
                    "odoo-dev-specs",
                    "-q",
                    "-f",
                    &path,
                ],
            )
            .context("Could not make the app's SSH key.")?;
        }
        Ok(fs::read_to_string(&public)
            .context("Could not read the app's SSH key.")?
            .trim()
            .to_string())
    }

    /// `git`, isolated like every tool, using only the app's key (when made) and
    /// never stopping to ask anything.
    pub(super) fn git(&self) -> Command {
        let mut cmd = isolated(&self.tools.git);
        cmd.env("GIT_TERMINAL_PROMPT", "0");
        let key = self.ssh_key_path();
        if key.is_file() {
            cmd.env(
                "GIT_SSH_COMMAND",
                format!(
                    "ssh -i '{}' -o IdentitiesOnly=yes -o BatchMode=yes -o ConnectTimeout=15 \
                     -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile='{}'",
                    key.display(),
                    self.ssh_dir().join("known_hosts").display()
                ),
            );
        }
        cmd
    }
}
