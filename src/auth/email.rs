//! Email sending for verification and password reset

use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

/// Email configuration
#[derive(Clone)]
pub struct EmailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_email: String,
    pub from_name: String,
    pub app_url: String,
}

impl EmailConfig {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            smtp_host: std::env::var("SMTP_HOST").ok()?,
            smtp_port: std::env::var("SMTP_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(587),
            smtp_username: std::env::var("SMTP_USERNAME").ok()?,
            smtp_password: std::env::var("SMTP_PASSWORD").ok()?,
            from_email: std::env::var("FROM_EMAIL").ok()?,
            from_name: std::env::var("FROM_NAME").unwrap_or_else(|_| "Take It Easy".to_string()),
            app_url: std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:3000".to_string()),
        })
    }
}

/// Email service
pub struct EmailService {
    config: EmailConfig,
    mailer: AsyncSmtpTransport<Tokio1Executor>,
}

impl EmailService {
    pub fn new(config: EmailConfig) -> Result<Self, lettre::transport::smtp::Error> {
        let creds = Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)?
            .port(config.smtp_port)
            .credentials(creds)
            .build();

        Ok(Self { config, mailer })
    }

    /// Send email verification
    pub async fn send_verification_email(
        &self,
        to_email: &str,
        username: &str,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let verify_url = format!("{}/auth/verify-email?token={}", self.config.app_url, token);

        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Verify your email</title>
</head>
<body style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h1 style="color: #333;">Welcome to Take It Easy, {}!</h1>
    <p>Thank you for registering. Please verify your email address by clicking the button below:</p>
    <p style="text-align: center; margin: 30px 0;">
        <a href="{}" style="background-color: #4CAF50; color: white; padding: 14px 28px; text-decoration: none; border-radius: 4px; display: inline-block;">
            Verify Email
        </a>
    </p>
    <p>Or copy and paste this link into your browser:</p>
    <p style="word-break: break-all; color: #666;">{}</p>
    <p style="color: #999; font-size: 12px; margin-top: 30px;">
        This link will expire in 24 hours. If you didn't create an account, you can safely ignore this email.
    </p>
</body>
</html>"#,
            username, verify_url, verify_url
        );

        let email = Message::builder()
            .from(
                format!("{} <{}>", self.config.from_name, self.config.from_email)
                    .parse()
                    .unwrap(),
            )
            .to(to_email.parse()?)
            .subject("Verify your email - Take It Easy")
            .header(ContentType::TEXT_HTML)
            .body(html_body)?;

        self.mailer.send(email).await?;
        Ok(())
    }

    /// Send password reset email
    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        username: &str,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let reset_url = format!("{}/auth/reset-password?token={}", self.config.app_url, token);

        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Reset your password</title>
</head>
<body style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h1 style="color: #333;">Password Reset Request</h1>
    <p>Hi {},</p>
    <p>We received a request to reset your password. Click the button below to create a new password:</p>
    <p style="text-align: center; margin: 30px 0;">
        <a href="{}" style="background-color: #2196F3; color: white; padding: 14px 28px; text-decoration: none; border-radius: 4px; display: inline-block;">
            Reset Password
        </a>
    </p>
    <p>Or copy and paste this link into your browser:</p>
    <p style="word-break: break-all; color: #666;">{}</p>
    <p style="color: #999; font-size: 12px; margin-top: 30px;">
        This link will expire in 1 hour. If you didn't request a password reset, you can safely ignore this email.
    </p>
</body>
</html>"#,
            username, reset_url, reset_url
        );

        let email = Message::builder()
            .from(
                format!("{} <{}>", self.config.from_name, self.config.from_email)
                    .parse()
                    .unwrap(),
            )
            .to(to_email.parse()?)
            .subject("Reset your password - Take It Easy")
            .header(ContentType::TEXT_HTML)
            .body(html_body)?;

        self.mailer.send(email).await?;
        Ok(())
    }
}

/// Mock email service for development/testing
pub struct MockEmailService {
    pub app_url: String,
}

impl MockEmailService {
    pub fn new(app_url: String) -> Self {
        Self { app_url }
    }

    pub async fn send_verification_email(
        &self,
        to_email: &str,
        username: &str,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let verify_url = format!("{}/auth/verify-email?token={}", self.app_url, token);
        log::info!(
            "[MOCK EMAIL] Verification email to {} ({}): {}",
            to_email,
            username,
            verify_url
        );
        Ok(())
    }

    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        username: &str,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let reset_url = format!("{}/auth/reset-password?token={}", self.app_url, token);
        log::info!(
            "[MOCK EMAIL] Password reset email to {} ({}): {}",
            to_email,
            username,
            reset_url
        );
        Ok(())
    }
}

/// Unified email sender trait
pub enum EmailSender {
    Real(EmailService),
    Mock(MockEmailService),
}

impl EmailSender {
    pub fn from_env() -> Self {
        match EmailConfig::from_env() {
            Some(config) => match EmailService::new(config) {
                Ok(service) => EmailSender::Real(service),
                Err(e) => {
                    log::warn!("Failed to initialize email service: {}. Using mock.", e);
                    let app_url = std::env::var("APP_URL")
                        .unwrap_or_else(|_| "http://localhost:3000".to_string());
                    EmailSender::Mock(MockEmailService::new(app_url))
                }
            },
            None => {
                log::info!("Email not configured. Using mock email service.");
                let app_url =
                    std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
                EmailSender::Mock(MockEmailService::new(app_url))
            }
        }
    }

    pub async fn send_verification_email(
        &self,
        to_email: &str,
        username: &str,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self {
            EmailSender::Real(service) => {
                service.send_verification_email(to_email, username, token).await
            }
            EmailSender::Mock(mock) => {
                mock.send_verification_email(to_email, username, token).await
            }
        }
    }

    pub async fn send_password_reset_email(
        &self,
        to_email: &str,
        username: &str,
        token: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self {
            EmailSender::Real(service) => {
                service.send_password_reset_email(to_email, username, token).await
            }
            EmailSender::Mock(mock) => {
                mock.send_password_reset_email(to_email, username, token).await
            }
        }
    }
}
