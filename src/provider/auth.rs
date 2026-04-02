use dialoguer::{Input, Password};
use grammers_client::Client;
use grammers_client::SignInError;

use crate::error::ProviderError;

pub async fn authenticate(client: &Client, api_hash: &str) -> Result<(), ProviderError> {
    if client.is_authorized().await.map_err(|e| {
        ProviderError::Auth(format!("Failed to check authorization: {e}"))
    })? {
        tracing::info!("Already authorized from session file");
        return Ok(());
    }

    tracing::info!("Not authorized, starting interactive login");

    let phone: String = Input::new()
        .with_prompt("Enter phone number (with country code)")
        .interact_text()
        .map_err(|e| ProviderError::Auth(format!("Failed to read phone: {e}")))?;

    let token = client
        .request_login_code(&phone, api_hash)
        .await
        .map_err(|e| ProviderError::Auth(format!("Failed to request login code: {e}")))?;

    let code: String = Input::new()
        .with_prompt("Enter the code you received")
        .interact_text()
        .map_err(|e| ProviderError::Auth(format!("Failed to read code: {e}")))?;

    match client.sign_in(&token, &code).await {
        Ok(_) => {
            tracing::info!("Signed in successfully");
            Ok(())
        }
        Err(SignInError::PasswordRequired(password_token)) => {
            let hint = password_token.hint().unwrap_or("none");
            let password = Password::new()
                .with_prompt(format!("Enter 2FA password (hint: {hint})"))
                .interact()
                .map_err(|e| ProviderError::Auth(format!("Failed to read password: {e}")))?;

            client
                .check_password(password_token, password.trim())
                .await
                .map_err(|e| ProviderError::Auth(format!("2FA failed: {e}")))?;

            tracing::info!("Signed in with 2FA successfully");
            Ok(())
        }
        Err(e) => Err(ProviderError::Auth(format!("Sign in failed: {e}"))),
    }
}
