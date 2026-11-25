use std::collections::HashSet;
use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use ticonnect_core::watch_party::valid_video_id;

const TIMEOUT: Duration = Duration::from_secs(8);
const MAX_THUMBNAIL_BYTES: usize = 512 * 1024;
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preview {
    pub video_id: String,
    pub title: String,
    pub channel: String,
    pub description: String,
    pub thumbnail_url: String,
    pub thumbnail_data: Option<String>,
    pub duration_seconds: u32,
}

#[derive(Deserialize)]
struct Oembed {
    title: String,
    author_name: String,
    thumbnail_url: String,
}

struct PageDetails {
    description: Option<String>,
    duration_seconds: Option<u32>,
}

pub async fn preview(video_id: &str) -> anyhow::Result<Preview> {
    if !valid_video_id(video_id) {
        anyhow::bail!("invalid video id");
    }
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()?;
    let watch_url = format!("https://www.youtube.com/watch?v={video_id}");
    let oembed_url = format!("https://www.youtube.com/oembed?url={watch_url}&format=json");
    let (oembed, details) = tokio::join!(
        fetch_oembed(&client, &oembed_url),
        fetch_page_details(&client, &watch_url),
    );
    let oembed = oembed?;
    let details = details.unwrap_or(PageDetails {
        description: None,
        duration_seconds: None,
    });
    let thumbnail_data = fetch_thumbnail_data(&client, video_id, &oembed.thumbnail_url).await;
    Ok(Preview {
        video_id: video_id.to_owned(),
        title: oembed.title,
        channel: oembed.author_name,
        description: details.description.unwrap_or_default(),
        thumbnail_url: oembed.thumbnail_url,
        thumbnail_data,
        duration_seconds: details.duration_seconds.unwrap_or_default(),
    })
}

async fn fetch_thumbnail_data(
    client: &reqwest::Client,
    video_id: &str,
    primary: &str,
) -> Option<String> {
    let candidates = [
        format!("https://i.ytimg.com/vi/{video_id}/maxresdefault.jpg"),
        primary.to_owned(),
        format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"),
        format!("https://i.ytimg.com/vi/{video_id}/mqdefault.jpg"),
        format!("https://img.youtube.com/vi/{video_id}/hqdefault.jpg"),
    ];
    let mut seen = HashSet::new();
    for url in candidates {
        if !seen.insert(url.clone()) {
            continue;
        }
        if let Some(data) = fetch_thumbnail(client, &url).await {
            return Some(data);
        }
    }
    None
}

async fn fetch_thumbnail(client: &reqwest::Client, url: &str) -> Option<String> {
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    if response
        .content_length()
        .is_some_and(|length| length as usize > MAX_THUMBNAIL_BYTES)
    {
        return None;
    }
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_owned();
    if !mime.starts_with("image/") {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() > MAX_THUMBNAIL_BYTES {
        return None;
    }
    Some(data_url(&mime, &bytes))
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", BASE64.encode(bytes))
}

pub async fn download(video_id: &str) -> anyhow::Result<std::path::PathBuf> {
    if !valid_video_id(video_id) {
        anyhow::bail!("invalid video id");
    }
    let path = std::env::temp_dir().join(format!("ticonnect-youtube-{video_id}.mp4"));
    if tokio::fs::metadata(&path)
        .await
        .is_ok_and(|metadata| metadata.len() > 0)
    {
        return Ok(path);
    }
    let _ = tokio::fs::remove_file(&path).await;
    let destination = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid temp path"))?;
    let binary = std::env::var("TICONNECT_YTDLP").unwrap_or_else(|_| "yt-dlp".to_owned());
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let output = tokio::time::timeout(
        Duration::from_secs(300),
        tokio::process::Command::new(binary)
            .args([
                "--no-warnings",
                "--no-playlist",
                "--no-progress",
                "-f",
                "18",
                "-o",
                destination,
                &url,
            ])
            .output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("yt-dlp timed out"))??;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "yt-dlp failed: {}",
            stderr.chars().take(300).collect::<String>()
        );
    }
    let metadata = tokio::fs::metadata(&path).await?;
    if metadata.len() == 0 {
        anyhow::bail!("yt-dlp produced an empty file");
    }
    Ok(path)
}

async fn fetch_oembed(client: &reqwest::Client, url: &str) -> anyhow::Result<Oembed> {
    let response = client.get(url).send().await?.error_for_status()?;
    Ok(response.json::<Oembed>().await?)
}

async fn fetch_page_details(client: &reqwest::Client, url: &str) -> Option<PageDetails> {
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let page = response.text().await.ok()?;
    Some(PageDetails {
        description: extract_description(&page),
        duration_seconds: json_string(&page, "lengthSeconds").and_then(|value| value.parse().ok()),
    })
}

fn extract_description(page: &str) -> Option<String> {
    json_string(page, "shortDescription").or_else(|| meta_content(page, "description"))
}

fn json_string(page: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\":\"");
    let start = page.find(&marker)? + marker.len();
    let rest = &page[start..];
    let mut escaped = false;
    let mut end = None;
    for (index, character) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => {
                end = Some(index);
                break;
            }
            _ => {}
        }
    }
    let raw = &rest[..end?];
    serde_json::from_str::<String>(&format!("\"{raw}\"")).ok()
}

fn meta_content(page: &str, name: &str) -> Option<String> {
    let marker = format!("<meta name=\"{name}\" content=\"");
    let start = page.find(&marker)? + marker.len();
    let rest = &page[start..];
    let end = rest.find('"')?;
    Some(decode_html(&rest[..end]))
}

fn decode_html(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_escaped_short_description() {
        let page = r#"<script>var ytInitialPlayerResponse = {"videoDetails":{"shortDescription":"Line one\nLine \"two\"","ownerChannelName":"TiConnect Labs"}};</script>"#;
        assert_eq!(
            extract_description(page).as_deref(),
            Some("Line one\nLine \"two\"")
        );
    }

    #[test]
    fn falls_back_to_meta_description() {
        let page =
            r#"<meta name="description" content="A &quot;short&quot; &amp; quiet walkthrough">"#;
        assert_eq!(
            extract_description(page).as_deref(),
            Some("A \"short\" & quiet walkthrough")
        );
    }

    #[test]
    fn ignores_missing_description() {
        assert_eq!(extract_description("<html></html>"), None);
    }

    #[test]
    fn formats_thumbnail_data_url() {
        assert_eq!(data_url("image/jpeg", b"hi"), "data:image/jpeg;base64,aGk=");
    }

    #[test]
    fn extracts_duration_seconds() {
        let page = r#""lengthSeconds":"213""#;
        assert_eq!(
            json_string(page, "lengthSeconds").and_then(|value| value.parse::<u32>().ok()),
            Some(213)
        );
    }
}
