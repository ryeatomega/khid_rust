use anyhow::{Context, Ok, Result};
use futures_util::{StreamExt, future::join_all};
use scraper::{Html, Selector};
use std::env;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufWriter};
use urlencoding::decode;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("[ERROR] No link provided.");
        println!("[USAGE] khid_rust.exe <link>.");
        Ok(())
    } else {
        let reqwest_client: reqwest::Client = reqwest::Client::new();
        let init_p_links: Result<Vec<String>, anyhow::Error> =
            init_page_scrape(&args[1], &reqwest_client)
                .await
                .context("[ERROR] Failed to get track links.");
        let down_p_links: Result<Vec<String>, anyhow::Error> =
            down_page_scrape(init_p_links.unwrap(), &reqwest_client)
                .await
                .context("[ERROR] Failed to get download links");
        let _ = download_tracks(down_p_links.unwrap(), &reqwest_client)
            .await
            .context("[ERROR] Failed to download the tracks.");
        Ok(())
    }
}

async fn fetch_html(link: &str, client: &reqwest::Client) -> Result<Html> {
    let response: reqwest::Response = client
        .get(link)
        .send()
        .await
        .context("[ERROR] GET request failed.")?;
    let status_code = response.status();
    if status_code == 200 {
        let html = response
            .text()
            .await
            .context("[ERROR] Failed to get HTML data.");
        let parsed_html = scraper::Html::parse_document(&html.unwrap());
        Ok(parsed_html)
    } else {
        eprintln!("[ERROR] HTTP {status_code} failed to fetch.");
        std::process::exit(1);
    }
}
async fn init_page_scrape(link: &str, client: &reqwest::Client) -> Result<Vec<String>> {
    println!("[Stage 1] Getting the album page.");
    let parsed_html = fetch_html(link, client).await?;
    let selector = Selector::parse(r#"td.playlistDownloadSong a[href*=".mp3"]"#)
        .expect("[ERROR] Failed to create selector");
    let links = parsed_html
        .select(&selector)
        .filter_map(|element| {
            element
                .value()
                .attr("href")
                .map(|href| format!("https://downloads.khinsider.com/{href}"))
        })
        .collect();
    Ok(links)
}
async fn down_page_scrape(downlist: Vec<String>, client: &reqwest::Client) -> Result<Vec<String>> {
    println!("[Stage 2] Getting the audio file links.");
    let html_list: Vec<Html> = join_all(downlist.iter().map(|link| fetch_html(&link, client)))
        .await
        .iter()
        .map(|result| result.as_ref().unwrap())
        .cloned()
        .collect();
    let selector: Selector = Selector::parse(r#"a:has(span.songDownloadLink)"#)
        .expect("[ERROR] Failed to create selector.");
    let links = html_list
        .iter()
        .map(|page: &Html| {
            page.select(&selector)
                .filter_map(|element: scraper::ElementRef<'_>| element.value().attr("href"))
                .filter(|href: &&str| href.ends_with(".mp3"))
                .map(|href| href.to_string())
                .collect()
        })
        .collect();
    Ok(links)
}
async fn download_tracks(links: Vec<String>, client: &reqwest::Client) -> Result<()> {
    println!("[Stage 3] Downloading files.");
    let mut stdin_handle = io::BufReader::new(io::stdin());
    'outer: for link in links {
        let track_name = decode(link.split("/").last().expect(
            "[ERROR] Failed to split the link, and get the last element (being the file name)",
        ))
        .expect("[ERROR] Failed to url decode the filename.")
        .into_owned();
        println!("[DOWNLOAD] \"{track_name}\" [y/n/q]?");
        let mut user_input_buffer = String::new();
        loop {
            user_input_buffer.clear();
            stdin_handle.read_line(&mut user_input_buffer).await?;
            match user_input_buffer.trim().to_lowercase().as_str() {
                "y" => break,
                "n" => continue 'outer,
                "q" => return Ok(()),
                _ => {
                    println!("[ERROR] Invalid option! Try again.")
                }
            }
        }

        let tokio_openoptions: tokio::fs::File = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&track_name)
            .await
            .expect("[ERROR] Failed to create OpenOptions (for BufWriter settings).");
        let mut buf_writer: BufWriter<tokio::fs::File> = BufWriter::new(tokio_openoptions);
        let target_file: reqwest::Response = client.get(&link).send().await?;
        let mut file_stream = target_file.bytes_stream();

        while let Some(a) = file_stream.next().await {
            let _ = buf_writer
                .write_all(&a.unwrap())
                .await
                .context("[ERROR] Failed to write stream chunk.");
        }
        println!("[INFO] File {0} downloaded successfully.", track_name);
    }
    Ok(())
}
