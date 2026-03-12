use std::net::{Ipv4Addr, Ipv6Addr};

use hickory_proto::{
    op::{Message, MessageType, ResponseCode},
    rr::{RData, Record, RecordType},
};
use tracing::{debug, trace};

use crate::utils::{AppError, Result};

pub struct DnsPacket;

impl DnsPacket {
    pub fn parse(buf: &[u8]) -> Result<Message> {
        let message = Message::from_vec(buf).map_err(AppError::Dns)?;
        Ok(message)
    }

    pub fn extract_domain(msg: &Message) -> Option<String> {
        let query = msg.queries().first()?;
        let name = query.name().to_string();

        Some(name.trim_end_matches('.').to_lowercase())
    }

    pub fn build_sinkhole(request: &Message) -> Result<Vec<u8>> {
        trace!("Building sinkhole response for query ID: {}", request.id());

        let mut response = Message::new();

        response.set_id(request.id());
        response.set_message_type(MessageType::Response);
        response.set_response_code(ResponseCode::NoError);

        for query in request.queries() {
            let mut record = Record::update0(query.name().clone(), 300, query.query_type());

            match query.query_type() {
                RecordType::A => {
                    record.set_data(RData::A(Ipv4Addr::new(0, 0, 0, 0).into()));
                    response.add_answer(record);
                }
                RecordType::AAAA => {
                    record.set_data(RData::AAAA(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).into()));
                    response.add_answer(record);
                }
                _ => {
                    debug!(
                        "Unhandled query type for blocked domain: {:?}",
                        query.query_type()
                    );
                }
            }
        }

        response.to_vec().map_err(AppError::Dns)
    }
}
