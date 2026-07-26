//! Markdown emitter for the step-capture guide: one heading and image
//! reference per Steps page. The images are saved beside the document
//! inside the same bundle directory, so references stay relative.

/// One exported step: the page's user-set name becomes the heading when
/// present, otherwise the step number does.
#[derive(Debug, Clone)]
pub struct GuideStep {
    pub title: Option<String>,
}

/// File stem (no extension) for a step's page image, 1-based.
pub fn guide_image_file_stem(step: usize) -> String {
    format!("step-{step:02}")
}

/// Render the guide document. Deterministic: the same steps always produce
/// the same bytes, so tests can assert on the exact output.
pub fn render_guide_markdown(steps: &[GuideStep]) -> String {
    let mut out = String::from("# Guide\n");
    for (index, step) in steps.iter().enumerate() {
        let number = index + 1;
        let title = step
            .title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map_or_else(|| format!("Step {number}"), str::to_string);
        let stem = guide_image_file_stem(number);
        out.push_str(&format!("\n## {title}\n\n![{title}]({stem}.png)\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_headings_fall_back_when_pages_are_unnamed() {
        let steps = [
            GuideStep {
                title: Some("Open the menu".to_string()),
            },
            GuideStep { title: None },
            GuideStep {
                title: Some("   ".to_string()),
            },
        ];

        assert_eq!(
            render_guide_markdown(&steps),
            "# Guide\n\
             \n## Open the menu\n\n![Open the menu](step-01.png)\n\
             \n## Step 2\n\n![Step 2](step-02.png)\n\
             \n## Step 3\n\n![Step 3](step-03.png)\n"
        );
    }

    #[test]
    fn an_empty_guide_is_just_the_heading() {
        assert_eq!(render_guide_markdown(&[]), "# Guide\n");
    }
}
