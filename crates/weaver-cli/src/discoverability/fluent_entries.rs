//! Test-only after-help catalogue builders used to assert localized help
//! output without widening the production discoverability surface.

pub(in crate::discoverability) const HEADER: (&str, &str) =
    ("weaver-after-help-header", "Domains and operations:");

fn domain_heading_entry(domain: super::KnownDomain) -> (String, String) {
    let description = super::DOMAIN_OPERATIONS
        .iter()
        .find(|(candidate, _, _)| *candidate == domain.as_str())
        .map(|(_, description, _)| *description)
        .unwrap_or_default();

    (
        format!("weaver-after-help-{}-heading", domain.as_str()),
        format!("{} \u{2014} {description}", domain.as_str()),
    )
}

fn pad_to(s: &mut String, width: usize) {
    if s.len() < width {
        s.extend(std::iter::repeat_n(' ', width - s.len()));
    }
}

fn format_operation_row(operations: &[String]) -> String {
    const SECOND_COLUMN_START: usize = 18;
    const THIRD_COLUMN_START: usize = 37;

    let mut row = String::new();
    if let Some(first) = operations.first() {
        row.push_str(first);
    }
    if let Some(second) = operations.get(1) {
        pad_to(&mut row, SECOND_COLUMN_START);
        row.push_str(second);
    }
    if let Some(third) = operations.get(2) {
        pad_to(&mut row, THIRD_COLUMN_START);
        row.push_str(third);
    }
    row
}

/// Renders the after-help domains-and-operations catalogue.
pub(crate) fn render_after_help(localizer: &dyn ortho_config::Localizer) -> String {
    let header = localizer.message(HEADER.0, None, HEADER.1);
    let mut sections = Vec::new();
    for (domain_str, _, operations) in super::DOMAIN_OPERATIONS {
        let domain = super::known_domain_from_catalogue_entry(domain_str);
        let (heading_id, heading_fallback) = domain_heading_entry(domain);
        let heading = localizer.message(&heading_id, None, &heading_fallback);
        let rows = operations
            .chunks(3)
            .map(|chunk| {
                let operations = chunk.iter().map(ToString::to_string).collect::<Vec<_>>();
                format!("    {}", format_operation_row(&operations))
            })
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("  {heading}\n{rows}"));
    }

    format!("{header}\n\n{}", sections.join("\n\n"))
}
