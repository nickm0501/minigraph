use crate::wal::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub id: u32,
    pub namespace: String,
    pub name: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PgOutputMessage {
    Relation(Relation),
    Insert {
        relation_id: u32,
        new_values: Vec<Value>,
    },
    Update {
        relation_id: u32,
        old_values: Option<Vec<Value>>,
        new_values: Vec<Value>,
    },
    Delete {
        relation_id: u32,
        old_values: Vec<Value>,
    },
}

#[derive(Debug)]
pub enum PgOutputError {
    Truncated(&'static str),
    InvalidUtf8(&'static str),
    UnsupportedTupleKind(u8),
    UnknownMessageTag(u8),
}

impl std::fmt::Display for PgOutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PgOutputError::Truncated(ctx) => write!(f, "pgoutput truncated while reading {ctx}"),
            PgOutputError::InvalidUtf8(ctx) => write!(f, "pgoutput invalid utf8 in {ctx}"),
            PgOutputError::UnsupportedTupleKind(tag) => {
                write!(f, "pgoutput unsupported tuple kind: {tag}")
            }
            PgOutputError::UnknownMessageTag(tag) => {
                write!(f, "pgoutput unknown message tag: {tag}")
            }
        }
    }
}

impl std::error::Error for PgOutputError {}

pub fn parse_pgoutput_messages(data: &[u8]) -> Result<Vec<PgOutputMessage>, PgOutputError> {
    let mut reader = Reader::new(data);
    let mut messages = Vec::new();

    while !reader.is_empty() {
        let tag = reader.read_u8("message tag")?;
        match tag {
            b'R' => messages.push(PgOutputMessage::Relation(parse_relation(&mut reader)?)),
            b'I' => messages.push(parse_insert(&mut reader)?),
            b'U' => messages.push(parse_update(&mut reader)?),
            b'D' => messages.push(parse_delete(&mut reader)?),
            other => return Err(PgOutputError::UnknownMessageTag(other)),
        }
    }

    Ok(messages)
}

fn parse_relation(reader: &mut Reader<'_>) -> Result<Relation, PgOutputError> {
    let id = reader.read_u32("relation id")?;
    let namespace = reader.read_cstring("relation namespace")?;
    let name = reader.read_cstring("relation name")?;

    // replica identity: we don't need the value right now, but it is part of the message.
    let _replica_identity = reader.read_u8("replica identity")?;

    let num_columns = reader.read_u16("relation column count")?;

    let mut columns = Vec::with_capacity(num_columns as usize);
    for _ in 0..num_columns {
        // flags
        let _flags = reader.read_u8("relation column flags")?;
        let col_name = reader.read_cstring("relation column name")?;

        // type OID + type modifier
        let _type_oid = reader.read_u32("relation column type oid")?;
        let _type_modifier = reader.read_i32("relation column type modifier")?;

        columns.push(col_name);
    }

    Ok(Relation {
        id,
        namespace,
        name,
        columns,
    })
}

fn parse_insert(reader: &mut Reader<'_>) -> Result<PgOutputMessage, PgOutputError> {
    let relation_id = reader.read_u32("insert relation id")?;

    let kind = reader.read_u8("insert tuple kind")?;
    if kind != b'N' {
        return Err(PgOutputError::UnknownMessageTag(kind));
    }

    let new_values = parse_tuple(reader)?;

    Ok(PgOutputMessage::Insert {
        relation_id,
        new_values,
    })
}

fn parse_update(reader: &mut Reader<'_>) -> Result<PgOutputMessage, PgOutputError> {
    let relation_id = reader.read_u32("update relation id")?;

    let mut old_values = None;

    let new_values = loop {
        let kind = reader.read_u8("update tuple kind")?;
        match kind {
            b'K' | b'O' => {
                old_values = Some(parse_tuple(reader)?);
            }
            b'N' => break parse_tuple(reader)?,
            other => return Err(PgOutputError::UnknownMessageTag(other)),
        }
    };

    Ok(PgOutputMessage::Update {
        relation_id,
        old_values,
        new_values,
    })
}

fn parse_delete(reader: &mut Reader<'_>) -> Result<PgOutputMessage, PgOutputError> {
    let relation_id = reader.read_u32("delete relation id")?;

    let kind = reader.read_u8("delete tuple kind")?;
    match kind {
        b'K' | b'O' => {}
        other => return Err(PgOutputError::UnknownMessageTag(other)),
    }

    let old_values = parse_tuple(reader)?;

    Ok(PgOutputMessage::Delete {
        relation_id,
        old_values,
    })
}

fn parse_tuple(reader: &mut Reader<'_>) -> Result<Vec<Value>, PgOutputError> {
    let num_columns = reader.read_u16("tuple column count")?;
    let mut values = Vec::with_capacity(num_columns as usize);

    for _ in 0..num_columns {
        let kind = reader.read_u8("tuple column kind")?;
        match kind {
            b'n' => values.push(Value::Null),
            b'u' => values.push(Value::Null),
            b't' | b'b' => {
                let len = reader.read_i32("tuple column length")?;
                if len < 0 {
                    return Err(PgOutputError::Truncated("tuple column bytes"));
                }

                let bytes = reader.read_bytes(len as usize, "tuple column bytes")?;

                // TODO: We currently treat both text and binary values as UTF-8 strings.
                // For Phase 1, we only need `documents.id` and `comments.document_id`, which
                // are sent as text under pgoutput defaults.
                let value = std::str::from_utf8(bytes)
                    .map_err(|_| PgOutputError::InvalidUtf8("tuple column"))?
                    .to_string();

                values.push(Value::Text(value));
            }
            other => return Err(PgOutputError::UnsupportedTupleKind(other)),
        }
    }

    Ok(values)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    #[cfg(test)]
    fn pos(&self) -> usize {
        self.pos
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn read_u8(&mut self, ctx: &'static str) -> Result<u8, PgOutputError> {
        let bytes = self.read_bytes(1, ctx)?;
        Ok(bytes[0])
    }

    fn read_u16(&mut self, ctx: &'static str) -> Result<u16, PgOutputError> {
        let bytes = self.read_bytes(2, ctx)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32(&mut self, ctx: &'static str) -> Result<u32, PgOutputError> {
        let bytes = self.read_bytes(4, ctx)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i32(&mut self, ctx: &'static str) -> Result<i32, PgOutputError> {
        let bytes = self.read_bytes(4, ctx)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_cstring(&mut self, ctx: &'static str) -> Result<String, PgOutputError> {
        let start = self.pos;
        let Some(end) = self.data[start..].iter().position(|b| *b == 0) else {
            return Err(PgOutputError::Truncated(ctx));
        };

        let end = start + end;
        let bytes = &self.data[start..end];
        self.pos = end + 1; // consume NUL terminator

        let s = std::str::from_utf8(bytes).map_err(|_| PgOutputError::InvalidUtf8(ctx))?;
        Ok(s.to_string())
    }

    fn read_bytes(&mut self, len: usize, ctx: &'static str) -> Result<&'a [u8], PgOutputError> {
        if self.pos + len > self.data.len() {
            return Err(PgOutputError::Truncated(ctx));
        }

        let bytes = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use super::*;

    const FIXTURE_REL_PATH: &str = "tests/fixtures/pgoutput_capture.bin";

    const DOC_ID: &str = "doc_fixture_pgoutput";
    const TEXT_INSERT_1: &str = "fixture insert 1";
    const TEXT_INSERT_2: &str = "fixture insert 2";
    const TEXT_OLD: &str = "fixture will update/delete";
    const TEXT_NEW: &str = "fixture updated";

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CommentRow {
        id: String,
        document_id: String,
        text: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CommentUpdate {
        old: CommentRow,
        new: CommentRow,
    }

    #[derive(Debug)]
    struct Fixture {
        messages: Vec<PgOutputMessage>,

        documents_relation: Relation,
        comments_relation: Relation,

        document_ids_inserted: Vec<String>,

        comment_inserts: Vec<CommentRow>,
        comment_updates: Vec<CommentUpdate>,
        comment_deletes: Vec<CommentRow>,
    }

    impl Fixture {
        fn load() -> Self {
            let bytes = fixture_bytes();
            let messages = parse_pgoutput_messages(bytes).expect("fixture pgoutput parse failed");

            if std::env::var("DUMP_PGOUTPUT_FIXTURE").is_ok() {
                eprintln!("pgoutput fixture messages: {messages:#?}");
            }

            use std::collections::HashMap;

            let mut relations_by_id: HashMap<u32, Relation> = HashMap::new();

            let mut documents_relation: Option<Relation> = None;
            let mut comments_relation: Option<Relation> = None;

            let mut document_ids_inserted = Vec::new();
            let mut comment_inserts = Vec::new();
            let mut comment_updates = Vec::new();
            let mut comment_deletes = Vec::new();

            for msg in &messages {
                match msg {
                    PgOutputMessage::Relation(rel) => {
                        relations_by_id.insert(rel.id, rel.clone());

                        match rel.name.as_str() {
                            "documents" => documents_relation = Some(rel.clone()),
                            "comments" => comments_relation = Some(rel.clone()),
                            _ => {}
                        }
                    }
                    PgOutputMessage::Insert {
                        relation_id,
                        new_values,
                    } => {
                        let rel = relations_by_id
                            .get(relation_id)
                            .expect("insert referenced unknown relation_id");

                        match rel.name.as_str() {
                            "documents" => {
                                let doc_id = expect_text(
                                    new_values.get(0).expect("documents insert missing id"),
                                    "documents.id",
                                );
                                document_ids_inserted.push(doc_id);
                            }
                            "comments" => {
                                comment_inserts.push(CommentRow::from_values(rel, new_values));
                            }
                            _ => {}
                        }
                    }
                    PgOutputMessage::Update {
                        relation_id,
                        old_values,
                        new_values,
                    } => {
                        let rel = relations_by_id
                            .get(relation_id)
                            .expect("update referenced unknown relation_id");

                        if rel.name != "comments" {
                            continue;
                        }

                        let old_values = old_values
                            .as_ref()
                            .expect("fixture expected replica identity FULL (old tuple present)");

                        comment_updates.push(CommentUpdate {
                            old: CommentRow::from_values(rel, old_values),
                            new: CommentRow::from_values(rel, new_values),
                        });
                    }
                    PgOutputMessage::Delete {
                        relation_id,
                        old_values,
                    } => {
                        let rel = relations_by_id
                            .get(relation_id)
                            .expect("delete referenced unknown relation_id");

                        if rel.name != "comments" {
                            continue;
                        }

                        comment_deletes.push(CommentRow::from_values(rel, old_values));
                    }
                }
            }

            Self {
                messages,
                documents_relation: documents_relation.expect("fixture missing documents relation"),
                comments_relation: comments_relation.expect("fixture missing comments relation"),
                document_ids_inserted,
                comment_inserts,
                comment_updates,
                comment_deletes,
            }
        }
    }

    impl CommentRow {
        fn from_values(rel: &Relation, values: &[Value]) -> Self {
            assert_eq!(
                values.len(),
                rel.columns.len(),
                "fixture tuple length mismatch for {}.{}",
                rel.namespace,
                rel.name
            );

            let id_idx = column_idx(rel, "id");
            let document_id_idx = column_idx(rel, "document_id");
            let text_idx = column_idx(rel, "text");

            Self {
                id: expect_text(&values[id_idx], "comments.id"),
                document_id: expect_text(&values[document_id_idx], "comments.document_id"),
                text: expect_text(&values[text_idx], "comments.text"),
            }
        }
    }

    fn column_idx(rel: &Relation, name: &str) -> usize {
        rel.columns
            .iter()
            .position(|col| col == name)
            .unwrap_or_else(|| panic!("fixture relation missing column {name}: {rel:?}"))
    }

    fn expect_text(value: &Value, ctx: &str) -> String {
        match value {
            Value::Text(s) => s.clone(),
            Value::Null => panic!("fixture expected text value for {ctx}"),
            Value::Int(_) => panic!("fixture expected text value for {ctx}"),
        }
    }

    fn fixture_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_REL_PATH)
    }

    fn fixture_bytes() -> &'static [u8] {
        static FIXTURE_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

        FIXTURE_BYTES
            .get_or_init(|| {
                let path = fixture_path();
                std::fs::read(&path).unwrap_or_else(|err| {
                    panic!(
                        "failed to read pgoutput fixture at {}: {err}",
                        path.display()
                    )
                })
            })
            .as_slice()
    }

    fn fixture() -> &'static Fixture {
        static FIXTURE: OnceLock<Fixture> = OnceLock::new();
        FIXTURE.get_or_init(Fixture::load)
    }

    #[test]
    fn parse_relation_message() {
        let fixture = fixture();

        assert_eq!(fixture.documents_relation.namespace, "public");
        assert_eq!(fixture.documents_relation.name, "documents");
        assert_eq!(fixture.documents_relation.columns, vec!["id"]);

        assert_eq!(fixture.comments_relation.namespace, "public");
        assert_eq!(fixture.comments_relation.name, "comments");
        assert_eq!(
            fixture.comments_relation.columns,
            vec!["id", "document_id", "text"]
        );
    }

    #[test]
    fn parse_insert_message() {
        let fixture = fixture();

        assert!(
            fixture.document_ids_inserted.iter().any(|id| id == DOC_ID),
            "fixture missing documents insert for {DOC_ID}"
        );

        let row_1 = fixture
            .comment_inserts
            .iter()
            .find(|row| row.document_id == DOC_ID && row.text == TEXT_INSERT_1)
            .expect("fixture missing expected comments insert 1");

        let row_2 = fixture
            .comment_inserts
            .iter()
            .find(|row| row.document_id == DOC_ID && row.text == TEXT_INSERT_2)
            .expect("fixture missing expected comments insert 2");

        uuid::Uuid::parse_str(&row_1.id).expect("comments.id should be a UUID string");
        uuid::Uuid::parse_str(&row_2.id).expect("comments.id should be a UUID string");
    }

    #[test]
    fn parse_update_message_with_old_and_new_tuple() {
        let fixture = fixture();

        let update = fixture
            .comment_updates
            .iter()
            .find(|u| {
                u.old.document_id == DOC_ID && u.old.text == TEXT_OLD && u.new.text == TEXT_NEW
            })
            .expect("fixture missing expected comments update");

        assert_eq!(update.old.id, update.new.id);
        assert_eq!(update.old.document_id, update.new.document_id);
    }

    #[test]
    fn parse_delete_message() {
        let fixture = fixture();

        let update = fixture
            .comment_updates
            .iter()
            .find(|u| {
                u.old.document_id == DOC_ID && u.old.text == TEXT_OLD && u.new.text == TEXT_NEW
            })
            .expect("fixture missing expected comments update");

        let deleted = fixture
            .comment_deletes
            .iter()
            .find(|row| row.id == update.new.id)
            .expect("fixture missing expected comments delete");

        assert_eq!(deleted.document_id, DOC_ID);
        assert_eq!(deleted.text, TEXT_NEW);
    }

    #[test]
    fn unknown_tag_errors() {
        let bytes = fixture_bytes();
        let mut corrupted = bytes.to_vec();
        corrupted[0] = b'X';

        let err = parse_pgoutput_messages(&corrupted).unwrap_err();
        assert!(matches!(err, PgOutputError::UnknownMessageTag(b'X')));
    }

    #[test]
    fn truncated_errors() {
        let bytes = fixture_bytes();
        assert_eq!(bytes[0], b'R', "fixture must start with a relation message");

        let err = parse_pgoutput_messages(&bytes[..1]).unwrap_err();
        assert!(matches!(err, PgOutputError::Truncated("relation id")));
    }

    #[test]
    fn invalid_utf8_errors() {
        let bytes = fixture_bytes();

        let mut reader = Reader::new(bytes);
        let tag = reader.read_u8("message tag").unwrap();
        assert_eq!(tag, b'R', "fixture must start with a relation message");
        let _rel_id = reader.read_u32("relation id").unwrap();

        let namespace_start = reader.pos();

        let mut corrupted = bytes.to_vec();
        corrupted[namespace_start] = 0xFF;

        let err = parse_pgoutput_messages(&corrupted).unwrap_err();
        assert!(matches!(
            err,
            PgOutputError::InvalidUtf8("relation namespace")
        ));
    }

    #[test]
    fn fixture_sanity_invariants() {
        let fixture = fixture();

        assert!(
            fixture
                .messages
                .iter()
                .any(|m| matches!(m, PgOutputMessage::Relation(_))),
            "fixture contains no relation messages"
        );
        assert!(
            fixture
                .messages
                .iter()
                .any(|m| matches!(m, PgOutputMessage::Insert { .. })),
            "fixture contains no insert messages"
        );
        assert!(
            fixture
                .messages
                .iter()
                .any(|m| matches!(m, PgOutputMessage::Update { .. })),
            "fixture contains no update messages"
        );
        assert!(
            fixture
                .messages
                .iter()
                .any(|m| matches!(m, PgOutputMessage::Delete { .. })),
            "fixture contains no delete messages"
        );
    }
}
