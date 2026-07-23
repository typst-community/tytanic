use std::path::Path;
use std::path::PathBuf;

use typst_syntax::LinkedNode;
use typst_syntax::SyntaxKind;
use typst_syntax::ast;

use super::Annotation;
use super::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocTestTag {
    Example,
    Test,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocTest {
    id: Id,
    source_path: PathBuf,
    function: String,
    block_index: usize,
    code: String,
    annotations: Vec<Annotation>,
    tag: DocTestTag,
}

impl DocTest {
    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn function(&self) -> &str {
        &self.function
    }

    pub fn block_index(&self) -> usize {
        self.block_index
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn tag(&self) -> DocTestTag {
        self.tag
    }

    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    pub fn is_skip(&self) -> bool {
        self.annotations.contains(&Annotation::Skip)
    }

    pub fn compile_only(&self) -> bool {
        true
    }

    pub fn parse(source_path: &Path, content: &str) -> Vec<DocTest> {
        let root = typst_syntax::parse(content);
        let root = LinkedNode::new(&root);

        let mut tests = Vec::new();
        collect_doc_tests(&root, content, source_path, &mut tests);
        tests
    }
}

fn collect_doc_tests(
    node: &LinkedNode,
    source: &str,
    source_path: &Path,
    tests: &mut Vec<DocTest>,
) {
    for child in node.children() {
        if child.kind() == SyntaxKind::LetBinding {
            if let Some((name, doc_lines)) = extract_let_docs(&child, source) {
                let blocks = parse_code_blocks(&doc_lines);
                for (i, block) in blocks.into_iter().enumerate() {
                    let id = Id::doc_test(source_path, &name, i);
                    tests.push(DocTest {
                        id,
                        source_path: source_path.to_path_buf(),
                        function: name.clone(),
                        block_index: i,
                        code: block.code,
                        annotations: Vec::new(),
                        tag: block.tag,
                    });
                }
            }
        }
        collect_doc_tests(&child, source, source_path, tests);
    }
}

fn extract_let_docs(node: &LinkedNode, source: &str) -> Option<(String, Vec<String>)> {
    let closure = node.children().find(|c| c.kind() == SyntaxKind::Closure)?;

    let name = closure
        .children()
        .find(|c| c.kind() == SyntaxKind::Ident)?;

    let name_text = &source[name.range()];

    let has_params = closure.children().any(|c| c.kind() == SyntaxKind::Params);
    if !has_params {
        return None;
    }

    let doc_lines = collect_doc_comment_lines(node);

    if doc_lines.is_empty() {
        return None;
    }

    Some((name_text.to_string(), doc_lines))
}

fn collect_doc_comment_lines(node: &LinkedNode) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    let mut current = node.clone();
    while let Some(prev) = current.prev_sibling_with_trivia() {
        if let Some(comment) = prev.get().cast::<ast::LineComment>() {
            if let Some(doc) = comment.text().strip_prefix('/') {
                lines.push(doc.strip_prefix(' ').unwrap_or(doc).to_string());
            } else {
                break;
            }
        } else if prev.get().cast::<ast::BlockComment>().is_some() {
            // skip block comments in doc test context
        } else if !matches!(prev.kind(), SyntaxKind::Space | SyntaxKind::Hash) {
            break;
        }
        current = prev;
    }

    if lines.is_empty() {
        return Vec::new();
    }

    lines.reverse();

    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    lines
}

struct DocBlock {
    code: String,
    tag: DocTestTag,
}

fn parse_code_blocks(lines: &[String]) -> Vec<DocBlock> {
    let mut blocks = Vec::new();
    let len = lines.len();
    let mut i = 0;

    while i < len {
        let trimmed = lines[i].trim();
        if let Some(tag_str) = trimmed.strip_prefix("```") {
            let tag_str = tag_str.trim();
            let tag = match tag_str {
                "example" => DocTestTag::Example,
                "test" => DocTestTag::Test,
                _ => {
                    i += 1;
                    continue;
                }
            };

            i += 1;
            let mut code = String::new();
            while i < len && !lines[i].trim().starts_with("```") {
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(&lines[i]);
                i += 1;
            }
            // Skip closing ```
            i += 1;

            if !code.trim().is_empty() {
                blocks.push(DocBlock { code, tag });
            }
        } else {
            i += 1;
        }
    }

    blocks
}