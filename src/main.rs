use anyhow::Result;
use scraper::Selector;

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::new();
    let a = init_page_scrape("https://downloads.khinsider.com/game-soundtracks/album/risk-of-rain-2-survivors-of-the-void-2022", &client).await?;
    let b = down_page_scrape(a, &client).await?;
    println!("{:#?}", b);
    Ok(())
}

async fn init_page_scrape(link: &'static str, client: &reqwest::Client) -> Result<Vec<String>> {
    let mut a: Vec<String> = Vec::new();
    let res: reqwest::Response = client.get(link).send().await.unwrap();
    println!("Status: {}", res.status());
    let html: String = res.text().await?;
    println!("Parsing...");
    let parsed: scraper::Html = scraper::Html::parse_document(&html);
    let selector: Selector = Selector::parse("td.playlistDownloadSong a[href]").unwrap();
    for element in parsed.select(&selector) {
        a.push(
            "https://downloads.khinsider.com/".to_string()
                + &element.value().attr("href").unwrap().to_string(),
        );
    }

    Ok(a)
}
async fn down_page_scrape(downlist: Vec<String>, client: &reqwest::Client) -> Result<Vec<String>> {
    let mut a: Vec<String> = Vec::new();

    for link in downlist {
        let res = client.get(link).send().await?;
        let html: String = res.text().await?;
        let parsed: scraper::Html = scraper::Html::parse_document(&html);
        let selector: Selector = Selector::parse("a[href]").unwrap();
        for element in parsed.select(&selector) {
            if let Some(href) = element.value().attr("href") {
                if href.ends_with(".flac") {
                    a.push(href.to_string());
                }
            }
        }
    }
    Ok(a)
}
