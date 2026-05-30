use torrentleech_cli::cli::{SearchSort, SortOrder};
use torrentleech_cli::error::ErrorKind;
use torrentleech_cli::search::{SearchRequest, build_search_url, parse_browse_html};
use url::Url;

fn base_url() -> Url {
    Url::parse("https://www.torrentleech.org/").unwrap()
}

#[test]
fn builds_search_url_with_site_ordered_path_segments() {
    let request = SearchRequest {
        query: Some("ubuntu iso".to_string()),
        categories: vec![33, 26],
        freeleech: true,
        page: 3,
        sort: SearchSort::Seeders,
        order: SortOrder::Asc,
    };

    let url = build_search_url(&base_url(), &request).unwrap();
    assert_eq!(
        url.path(),
        "/torrents/browse/index/categories/33,26/facets/tags%253AFREELEECH/query/ubuntu%20iso/page/3/orderby/seeders/order/asc"
    );
}

#[test]
fn parses_browse_table_results_and_metadata() {
    let html = include_str!("fixtures/search/browse.html");
    let page = parse_browse_html(html, &base_url()).unwrap();

    assert_eq!(page.total, Some(42));
    assert!(page.has_next_page);
    assert_eq!(page.results.len(), 2);

    let first = &page.results[0];
    assert_eq!(first.id, 1001);
    assert_eq!(first.title, "Example.Release.One");
    assert_eq!(first.category_id, Some(33));
    assert_eq!(first.category.as_deref(), Some("0-day"));
    assert_eq!(first.tags, ["FREELEECH", "Internal"]);
    assert_eq!(first.added.as_deref(), Some("2026-05-29 10:15:00"));
    assert_eq!(first.comments, Some(12));
    assert_eq!(first.size.as_deref(), Some("1.5 GiB"));
    assert_eq!(first.completed, Some(123));
    assert_eq!(first.seeders, Some(45));
    assert_eq!(first.leechers, Some(6));
    assert_eq!(first.uploader.as_deref(), Some("uploader-one"));
    assert!(first.freeleech);
    assert_eq!(
        first.download_url.as_str(),
        "https://www.torrentleech.org/download/1001/Example.Release.One.torrent"
    );

    let second = &page.results[1];
    assert_eq!(second.id, 1002);
    assert_eq!(second.category_id, Some(26));
    assert!(second.tags.is_empty());
    assert_eq!(second.uploader, None);
    assert!(!second.freeleech);
    assert_eq!(
        second.download_url.as_str(),
        "https://www.torrentleech.org/download/1002/Example.Release.Two.torrent"
    );
}

#[test]
fn parses_zero_results_without_next_page() {
    let html = include_str!("fixtures/search/zero.html");
    let page = parse_browse_html(html, &base_url()).unwrap();

    assert_eq!(page.total, Some(0));
    assert!(!page.has_next_page);
    assert!(page.results.is_empty());
}

#[test]
fn malformed_browse_rows_return_parse_failure() {
    let html = include_str!("fixtures/search/malformed.html");
    let error = parse_browse_html(html, &base_url()).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::ParseFailure);
}

#[test]
fn missing_browse_table_returns_parse_failure() {
    let error = parse_browse_html("<html><body>login</body></html>", &base_url()).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::ParseFailure);
}

#[test]
fn next_title_link_does_not_set_next_page_state() {
    let html = r#"
        <html>
          <body>
            <div class="browse-header">Showing 1 - 1 of 1 result</div>
            <table id="torrenttable">
              <tbody>
                <tr>
                  <td>
                    <a class="title" href="/torrent/1001">Next</a>
                    <a class="download" href="/download/1001/Next.torrent">Download</a>
                  </td>
                  <td>2026-05-29 10:15:00</td>
                  <td>0</td>
                  <td>1 MiB</td>
                  <td>1</td>
                  <td>1</td>
                  <td>1</td>
                </tr>
              </tbody>
            </table>
          </body>
        </html>
    "#;

    let page = parse_browse_html(html, &base_url()).unwrap();
    assert!(!page.has_next_page);
}

#[test]
fn sort_fields_use_site_orderby_names() {
    let request = SearchRequest {
        query: None,
        categories: Vec::new(),
        freeleech: false,
        page: 1,
        sort: SearchSort::Name,
        order: SortOrder::Desc,
    };

    let url = build_search_url(&base_url(), &request).unwrap();
    assert_eq!(
        url.path(),
        "/torrents/browse/index/page/1/orderby/nameSort/order/desc"
    );

    let request = SearchRequest {
        sort: SearchSort::Comments,
        ..request
    };
    let url = build_search_url(&base_url(), &request).unwrap();
    assert_eq!(
        url.path(),
        "/torrents/browse/index/page/1/orderby/numComments/order/desc"
    );
}

#[test]
fn total_count_accepts_thousands_separators() {
    let html = r#"
        <html>
          <body>
            <div class="browse-header">Showing 1 - 25 of 1,234 results</div>
            <table id="torrenttable">
              <tbody>
                <tr>
                  <td>
                    <a class="title" href="/torrent/1001">Example</a>
                    <a class="download" href="/download/1001/Example.torrent">Download</a>
                  </td>
                  <td>2026-05-29 10:15:00</td>
                  <td>0</td>
                  <td>1 MiB</td>
                  <td>1</td>
                  <td>1</td>
                  <td>1</td>
                </tr>
              </tbody>
            </table>
          </body>
        </html>
    "#;

    let page = parse_browse_html(html, &base_url()).unwrap();
    assert_eq!(page.total, Some(1234));
}

#[test]
fn total_count_accepts_found_torrents_sentence() {
    let html = r#"
        <html>
          <body>
            <div class="browse-header">Found 394 torrents.</div>
            <table id="torrenttable">
              <tbody>
                <tr>
                  <td>
                    <a class="title" href="/torrent/1001">Example</a>
                    <a class="download" href="/download/1001/Example.torrent">Download</a>
                  </td>
                  <td>2026-05-29 10:15:00</td>
                  <td>0</td>
                  <td>1 MiB</td>
                  <td>1</td>
                  <td>1</td>
                  <td>1</td>
                </tr>
              </tbody>
            </table>
          </body>
        </html>
    "#;

    let page = parse_browse_html(html, &base_url()).unwrap();
    assert_eq!(page.total, Some(394));
}
