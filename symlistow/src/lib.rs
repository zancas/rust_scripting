use std::fs;
use std::io::{self, Write};
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

/// Err Type for failed verification
#[derive(Debug, thiserror::Error)]
enum VerificationError {
    #[error("Path doesn't exist: {0}")]
    MissingPath(PathBuf),
    #[error("Failed call to --version, got {0}")]
    VersionCallFail(ExitStatus),
    #[error("Binary did not execute successfully: {0}")]
    ExecutionError(#[from] io::Error),
}

/// Verifies that a binary exists and can run --version.
/// Returns Ok with the version string on success, or Err with an error message.
fn verify_binary(binary_path: &str, binary_name: &str) -> Result<String, VerificationError> {
    if !Path::new(binary_path).exists() {
        return Err(VerificationError::MissingPath(binary_path.into()));
    }

    // Try to run the version command
    let result = Command::new(binary_path).arg("--version").output();

    match result {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                Ok(version.trim().to_string())
            } else {
                Err(VerificationError::VersionCallFail(output.status))
            }
        }
        Err(e) => Err(VerificationError::ExecutionError(e)),
    }
}

/// Prompts the user to decide whether to replace an existing binary.
/// Returns true if the user wants to replace, false otherwise.
fn prompt_user_for_replacement(
    binary_name: &str,
    existing_version: &str,
    new_version: &str,
) -> bool {
    println!("\n{} version mismatch detected:", binary_name);
    println!("  Existing: {}", existing_version);
    println!("  New:      {}", new_version);

    loop {
        print!("Replace existing with new? [Y/n]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "" | "y" | "yes" => return true,
            "n" | "no" => return false,
            _ => {
                println!("Invalid input. Please enter 'y' for yes or 'n' for no.");
                continue;
            }
        }
    }
}

/// Handles creating or updating a symlink for a binary.
///
/// # Arguments
/// * `link_path` - The path where the symlink should be created
/// * `source_path` - The path to the actual binary file
/// * `binary_name` - The name of the binary (for display purposes)
/// * `source_version` - The version string of the source binary
/// * `interactive` - Whether to prompt the user for confirmation on replacements
pub fn handle_symlink(
    link_path: &Path,
    source_path: &str,
    binary_name: &str,
    source_version: &str,
    interactive: bool,
) {
    if link_path.exists() {
        // Verify existing symlink/binary
        match verify_binary(
            link_path.to_str().unwrap(),
            &format!("{} (existing)", binary_name),
        ) {
            Ok(existing_version) => {
                if existing_version == source_version {
                    println!("✓ {} symlink already exists with same version", binary_name);
                    return;
                }

                // Different versions - prompt user or force replace
                let should_replace = if !interactive {
                    println!(
                        "Non-interactive mode: forcing replacement of {}",
                        binary_name
                    );
                    true
                } else {
                    prompt_user_for_replacement(binary_name, &existing_version, source_version)
                };

                if should_replace {
                    println!("Replacing {} symlink...", binary_name);
                    if let Err(e) = fs::remove_file(link_path) {
                        eprintln!("Error: Failed to remove existing {}: {}", binary_name, e);
                        return;
                    }
                    if let Err(e) = unix_fs::symlink(source_path, link_path) {
                        eprintln!("Error: Failed to create {} symlink: {}", binary_name, e);
                    } else {
                        println!("✓ {} symlink replaced successfully", binary_name);
                    }
                } else {
                    println!("Keeping existing {} symlink", binary_name);
                }
            }
            Err(e) => {
                eprintln!("Warning: Existing {} is invalid: {}", binary_name, e);
                eprintln!("Removing and recreating symlink...");
                let _ = fs::remove_file(link_path);
                if let Err(e) = unix_fs::symlink(source_path, link_path) {
                    eprintln!("Error: Failed to create {} symlink: {}", binary_name, e);
                } else {
                    println!("✓ {} symlink created successfully", binary_name);
                }
            }
        }
    } else {
        // No existing symlink - create it
        println!("Creating symlink for {}...", binary_name);
        if let Err(e) = unix_fs::symlink(source_path, link_path) {
            eprintln!("Error: Failed to create {} symlink: {}", binary_name, e);
        } else {
            println!("✓ {} symlink created successfully", binary_name);
        }
    }
}

/// Verifies a binary exists and can execute, then adds its information to a collection.
///
/// # Arguments
/// * `path` - The file path to the binary
/// * `versions` - A mutable vector to collect the binary information (path, version)
///
/// # Returns
/// * `true` if the binary was successfully verified and added to the collection
/// * `false` if the binary could not be verified
pub fn append_verified_binaries(path: &str, versions: &mut Vec<(String, String)>) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    match verify_binary(path, name) {
        Ok(version) => {
            println!("✓ {}: {}", name, version);
            versions.push((path.to_string(), version));
            true
        }
        Err(e) => {
            eprintln!("✗ {}", e);
            false
        }
    }
}
