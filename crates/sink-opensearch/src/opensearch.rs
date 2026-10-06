//! Writes canonical events to OpenSearch through the `_bulk` API.

mod bulk;
mod classify;
mod document;
mod org_index;
mod response;

use std::future::Future;

use bytes::BytesMut;

use bulk::BulkClient;
pub use bulk::BulkClientError;
use document::BulkDoc;
use org_index::OrgIndexes;

use crate::cac::Cac;
use crate::config::OpenSearchConfig;
use crate::runtime::{ItemOutcome, Rejection, WriteError, Writer};

pub struct OpenSearchWriter {
    client: BulkClient,
    indexes: OrgIndexes,
    max_doc_bytes: usize,
}

impl OpenSearchWriter {
    pub fn new(
        config: &OpenSearchConfig,
        cac: Cac,
        max_doc_bytes: usize,
    ) -> Result<Self, BulkClientError> {
        Ok(Self {
            client: BulkClient::new(config)?,
            indexes: OrgIndexes::new(cac, config.index_from_env.then(|| config.index.clone())),
            max_doc_bytes,
        })
    }
}

impl Writer for OpenSearchWriter {
    type Doc = BulkDoc;

    async fn prepare(&self, payload: Option<&[u8]>) -> Result<BulkDoc, Rejection> {
        let decoded = document::decode(payload)?;
        let event = &decoded.event;
        let index = self
            .indexes
            .index_for(&event.org_id, event.proj_id.as_deref())
            .await
            .map_err(|reason| {
                Rejection::new("invalid_index", format!("org `{}`: {reason}", event.org_id))
            })?;
        document::build(&decoded, &index, self.max_doc_bytes)
    }

    fn doc_size(doc: &BulkDoc) -> usize {
        doc.len()
    }

    fn write(
        &self,
        docs: Vec<BulkDoc>,
    ) -> impl Future<Output = Result<Vec<ItemOutcome>, WriteError>> + Send {
        let mut body = BytesMut::with_capacity(docs.iter().map(BulkDoc::len).sum());
        for doc in &docs {
            body.extend_from_slice(doc.as_bytes());
        }
        self.client.send(body.freeze(), docs.len())
    }
}
