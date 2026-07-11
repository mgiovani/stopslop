use crate::{context::TextNode, diagnostic::Diagnostic};
use std::collections::HashSet;

pub fn apply(diags: &mut Vec<Diagnostic>, comments: &[TextNode]) {
    let mut lines: HashSet<usize> = HashSet::new();
    let mut file_wide = false;
    for c in comments {
        if c.text.contains("ai-slop-ignore-file") {
            file_wide = true;
        } else if c.text.contains("ai-slop-ignore") {
            lines.insert(c.line); // finding on the SAME line as the comment
                                  // Comment may span multiple lines (block comment); "below" means below its
                                  // closing delimiter, not one past its start line.
            let end_line = c.line + c.text.matches('\n').count();
            lines.insert(end_line + 1); // finding on the line BELOW the comment
        }
    }
    if file_wide {
        diags.clear();
        return;
    }
    diags.retain(|d| !lines.contains(&d.line));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{Diagnostic, Tier};

    fn diag(line: usize) -> Diagnostic {
        Diagnostic {
            code: "SLOP004",
            name: "test",
            tier: Tier::A,
            path: "f.ts".into(),
            line,
            col: 1,
            message: "test".into(),
        }
    }

    /// Multi-line block comment starting at line 2, ending at line 4 (`\n` x2), must
    /// suppress a finding on line 5 (the line below its closing `*/`), not line 3.
    #[test]
    fn multiline_block_comment_suppresses_line_after_its_end() {
        let comment = TextNode {
            text: "/* ai-slop-ignore\n   multi-line note\n*/",
            start_byte: 0,
            end_byte: 40,
            line: 2,
            col: 1,
            is_doc: false,
        };
        let mut diags = vec![diag(5)];
        apply(&mut diags, &[comment]);
        assert!(
            diags.is_empty(),
            "line below a multi-line comment's end should be suppressed"
        );
    }
}
