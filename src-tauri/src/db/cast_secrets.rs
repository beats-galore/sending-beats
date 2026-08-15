// Stream passwords, kept in the keychain rather than in the database
//
// Everything else about a station is ordinary configuration and belongs in a
// file that can be copied, inspected and backed up. A source password is not:
// it is a credential to someone else's server, and a database carrying one
// cannot be handed around without handing that over too.
//
// Keyed by the configuration's row id, so the secret and the row are written and
// forgotten together, and renaming a station does not strand its password.

use anyhow::{Context, Result};

/// What these entries are filed under in the keychain
const KEYCHAIN_SERVICE: &str = "com.SweetBeatsStudio.cast";

/// The account a station's password is stored against
fn account_for(configuration_id: &str) -> String {
    format!("cast:{}", configuration_id)
}

/// Store or replace a station's password
///
/// An empty password clears the entry rather than storing nothing, so "no
/// password" is one state rather than two that read differently.
#[cfg(target_os = "macos")]
pub fn set_password(configuration_id: &str, password: &str) -> Result<()> {
    if password.is_empty() {
        return forget_password(configuration_id);
    }

    security_framework::passwords::set_generic_password(
        KEYCHAIN_SERVICE,
        &account_for(configuration_id),
        password.as_bytes(),
    )
    .context("Could not save the stream password to the keychain")
}

/// Read a station's password back, or None when it has never been given one
#[cfg(target_os = "macos")]
pub fn password(configuration_id: &str) -> Result<Option<String>> {
    match security_framework::passwords::get_generic_password(
        KEYCHAIN_SERVICE,
        &account_for(configuration_id),
    ) {
        Ok(secret) => Ok(Some(
            String::from_utf8(secret).context("The stored stream password is not text")?,
        )),
        // Absent is the ordinary case for a station that has not been given one,
        // and is not worth reporting as a failure.
        Err(_) => Ok(None),
    }
}

/// Whether a station has a password stored, without reading it
///
/// What the interface asks: a password field can say it is set without the
/// secret being sent to the front end to say so.
#[cfg(target_os = "macos")]
pub fn has_password(configuration_id: &str) -> bool {
    password(configuration_id).ok().flatten().is_some()
}

#[cfg(target_os = "macos")]
pub fn forget_password(configuration_id: &str) -> Result<()> {
    // Deleting one that was never there is the same outcome as deleting one that
    // was, so a missing entry is not an error to report.
    let _ = security_framework::passwords::delete_generic_password(
        KEYCHAIN_SERVICE,
        &account_for(configuration_id),
    );

    Ok(())
}

// Nothing else is built for another platform yet, and a stub that silently
// dropped a password would be worse than not compiling.
#[cfg(not(target_os = "macos"))]
pub fn set_password(_configuration_id: &str, _password: &str) -> Result<()> {
    Err(anyhow::anyhow!(
        "Storing stream passwords is only implemented on macOS"
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn password(_configuration_id: &str) -> Result<Option<String>> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub fn has_password(_configuration_id: &str) -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn forget_password(_configuration_id: &str) -> Result<()> {
    Ok(())
}
