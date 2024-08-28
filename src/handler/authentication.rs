use axum::{extract::State, Json};
use lettre::{
    transport::smtp::authentication::Credentials, Message,
    message::{header, SinglePart},
    SmtpTransport, Transport, message::Mailbox, Address
};
use reqwest::StatusCode;
use serde::Deserialize;

use crate::repository::Repository;



#[derive(Deserialize)]
pub struct SignUpRequest {
    email: String,
}

pub async fn sign_up_request(
    State(state): State<Repository>,
    Json(body): Json<SignUpRequest>,
) -> Result<StatusCode, StatusCode> {
    let user_address = body.email.parse::<Address>().map_err(|_| StatusCode::BAD_REQUEST)?;

    let app_address = std::env::var("MAIL_ADDRESS").unwrap();
    let app_password = std::env::var("MAIL_PASSWORD").unwrap();
    let smtp = "smtp.gmail.com";
    let jwt = state
        .make_jwt_and_save(&body.email)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let message = format!("これはテストメールです。
    以下のリンクをクリックしてください。
    https://link/{jwt}");


    let email = Message::builder()
        .from(Mailbox::new(Some("traOJudge".to_owned()), app_address.parse::<Address>().unwrap()))
        .to(Mailbox::new(None, user_address))
        .subject("テストメール")
        .singlepart(
            SinglePart::builder()
                .header(header::ContentType::TEXT_PLAIN)
                .body(message.to_owned()),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let credentials = Credentials::new(app_address, app_password);


    let mailer = SmtpTransport::starttls_relay(smtp)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .credentials(credentials)
        .build();

    match mailer.send(&email) {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}