//! Writes canonical events to OpenSearch through the `_bulk` API.

mod bulk;
mod classify;
mod document;
mod response;

use std::future::Future;

use bytes::BytesMut;

use bulk::BulkClient;
pub use bulk::BulkClientError;
use document::BulkDoc;

use crate::config::{IndexTemplate, OpenSearchConfig};
use crate::runtime::{ItemOutcome, Rejection, WriteError, Writer};

pub struct OpenSearchWriter {
    client: BulkClient,
    index: IndexTemplate,
    max_doc_bytes: usize,
}

impl OpenSearchWriter {
    pub fn new(config: &OpenSearchConfig, max_doc_bytes: usize) -> Result<Self, BulkClientError> {
        Ok(Self {
            client: BulkClient::new(config)?,
            index: config.index.clone(),
            max_doc_bytes,
        })
    }
}

impl Writer for OpenSearchWriter {
    type Doc = BulkDoc;

    fn prepare(&self, payload: Option<&[u8]>) -> Result<BulkDoc, Rejection> {
        document::prepare(payload, &self.index, self.max_doc_bytes)
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
