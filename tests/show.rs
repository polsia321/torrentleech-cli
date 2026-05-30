use torrentleech_cli::show::parse_torrent_details;
use url::Url;

fn base_url() -> Url {
    Url::parse("https://www.torrentleech.org").unwrap()
}

#[test]
fn parses_torrent_details_from_detail_page() {
    let html = include_str!("fixtures/show/detail.html");
    let details = parse_torrent_details(html, "123", &base_url()).unwrap();

    assert_eq!(details.id, 123);
    assert_eq!(details.title, "Example.Release");
    assert_eq!(details.category.as_deref(), Some("0-day"));
    assert_eq!(
        details.added.as_deref(),
        Some("Wednesday 27th May 2026 07:10:38 AM")
    );
    assert_eq!(details.size.as_deref(), Some("6.75 GB"));
    assert_eq!(details.seeders, Some(111));
    assert_eq!(details.leechers, Some(23));
    assert_eq!(details.completed, Some(268));
    assert_eq!(details.uploader.as_deref(), Some("Anonymous"));
    assert_eq!(details.tags, vec!["FREELEECH"]);
    assert_eq!(details.description.as_deref(), Some("Line one Line two"));
    assert_eq!(details.nfo.as_deref(), Some("NFO line 1\nNFO line 2"));
    assert_eq!(
        details.download_url.as_str(),
        "https://www.torrentleech.org/download/123/example.release.torrent"
    );
}
