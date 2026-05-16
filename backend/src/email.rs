use anyhow::Result;
use lettre::{
    message::{header::ContentType, Attachment, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

const WAPPEN: &[u8] = include_bytes!("../../frontend/static/wappen.png");

const TEMPLATE: &str = include_str!("../templates/confirmation.eml");
const TEMPLATE_PLAIN: &str = include_str!("../templates/confirmation.txt");

#[derive(Clone)]
pub struct EmailConfig {
    pub from: lettre::message::Mailbox,
    pub mailer: AsyncSmtpTransport<Tokio1Executor>,
}

impl EmailConfig {
    pub fn new(
        smtp_host: &str,
        smtp_port: u16,
        smtp_user: &str,
        smtp_password: &str,
        from: &str,
    ) -> Result<Self> {
        let creds = Credentials::new(smtp_user.to_string(), smtp_password.to_string());
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?
            .credentials(creds)
            .port(smtp_port)
            .build();
        Ok(Self {
            from: from.parse()?,
            mailer,
        })
    }
}

pub async fn send_confirmation(config: &EmailConfig, to: &str, name: &str, slot: &str) -> Result<()> {
    let (subject, html, plain) = parse_template(name, slot)?;

    let html_part = SinglePart::builder()
        .header(ContentType::TEXT_HTML)
        .body(html);

    let wappen_part = Attachment::new_inline("wappen".to_string())
        .body(WAPPEN.to_vec(), "image/png".parse().unwrap());

    let related = MultiPart::related()
        .singlepart(html_part)
        .singlepart(wappen_part);

    let plain_part = SinglePart::builder()
        .header(ContentType::TEXT_PLAIN)
        .body(plain);

    let email = Message::builder()
        .from(config.from.clone())
        .to(to.parse()?)
        .bcc(config.from.clone())
        .subject(subject)
        .multipart(MultiPart::alternative().singlepart(plain_part).multipart(related))?;

    config.mailer.send(email).await?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn parse_template(name: &str, slot: &str) -> Result<(String, String, String)> {
    let (headers, body) = TEMPLATE
        .split_once("\n\n")
        .ok_or_else(|| anyhow::anyhow!("email template missing blank line between headers and body"))?;

    let subject = headers
        .lines()
        .find_map(|line| line.strip_prefix("Subject:"))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| anyhow::anyhow!("email template missing Subject header"))?;

    let name_escaped = html_escape(name);
    let subject = subject.replace("{{name}}", &name_escaped).replace("{{slot}}", slot);
    let html = body.replace("{{name}}", &name_escaped).replace("{{slot}}", slot);
    let plain = TEMPLATE_PLAIN.replace("{{name}}", name).replace("{{slot}}", slot);

    Ok((subject, html, plain))
}
