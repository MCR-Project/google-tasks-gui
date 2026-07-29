use keyring::Entry;

const SERVICE_NAME: &str = "gtasks-tui";
const REFRESH_TOKEN_KEY: &str = "refresh_token";

// Save the refresh token in the linux system keyring
pub fn save_refresh_token(token: &str) -> Result<(), keyring::Error> {
    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_KEY)?;
    entry.set_password(token)?;
    Ok(())
}

// Retrieve the refresh token from the linux system keyring
pub fn get_refresh_token() -> Result<String, keyring::Error> {
    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_KEY)?;
    entry.get_password()
}

// Delete the refresh token from the linux system keyring
pub fn delete_refresh_token() -> Result<(), keyring::Error> {
    let entry = Entry::new(SERVICE_NAME, REFRESH_TOKEN_KEY)?;
    entry.delete_password()?;
    Ok(())
}
