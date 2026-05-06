use anyhow::Result;
use runglass_core::RunReport;

use crate::script::SCRIPT;
use crate::style::STYLE;

pub fn render_html(report: &RunReport) -> Result<String> {
    let json = serde_json::to_string(report)?;
    Ok(format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>RunGlass Receipt</title>
  <link rel="icon" type="image/svg+xml" href="/assets/runglass_icon.svg" />
  <link rel="icon" type="image/png" sizes="32x32" href="/assets/runglass_favicon_32.png" />
  <link rel="apple-touch-icon" sizes="180x180" href="/assets/runglass_apple_touch.png" />
  <style>{style}</style>
</head>
<body>
  <div id="app"></div>
  <script id="report-data" type="application/json">{json}</script>
  <script>{script}</script>
</body>
</html>"#,
        style = STYLE,
        json = json,
        script = SCRIPT,
    ))
}

#[cfg(test)]
mod tests {
    use super::render_html;
    use runglass_core::fixture::sample_report;

    #[test]
    fn html_export_embeds_receipt_data_and_app_shell() {
        let report = sample_report("html-export-test".to_string());
        let html = render_html(&report).expect("render html");

        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("<title>RunGlass Receipt</title>"));
        assert!(html.contains("id=\"report-data\""));
        assert!(html.contains("docker compose up -d"));
        assert!(html.contains("/assets/runglass_icon.svg"));
        assert!(html.contains("/assets/runglass_favicon_32.png"));
    }
}
