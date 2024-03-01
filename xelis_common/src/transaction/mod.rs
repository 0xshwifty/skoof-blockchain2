use crate::{
    crypto::{
        elgamal::{CompressedCommitment, CompressedHandle, CompressedPublicKey},
        Signature,
        Hashable,
        Hash
    },
    serializer::{Serializer, Writer, Reader, ReaderError}
};
use log::debug;
use serde::{Deserialize, Serialize};

// Maximum size of payload per transfer
pub const EXTRA_DATA_LIMIT_SIZE: usize = 1024;
pub const MAX_TRANSFER_COUNT: usize = 255;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransferPayload {
    asset: Hash,
    destination: CompressedPublicKey,
    // we can put whatever we want up to EXTRA_DATA_LIMIT_SIZE bytes
    extra_data: Option<Vec<u8>>,
    /// Represents the ciphertext along with `sender_handle` and `receiver_handle`.
    /// The opening is reused for both of the sender and receiver commitments.
    commitment: CompressedCommitment,
    sender_handle: CompressedHandle,
    receiver_handle: CompressedHandle,
    // ct_validity_proof: CiphertextValidityProof,
}

// Burn is a public payload allowing to use it as a proof of burn
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BurnPayload {
    asset: Hash,
    amount: u64
}

// this enum represent all types of transaction available on XELIS Network
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Transfers(Vec<TransferPayload>),
    Burn(BurnPayload),
}

// Compressed transaction to be sent over the network
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Transaction {
    // Version of the transaction
    // This is for future use
    version: u8,
    // source of the assets being sent
    source: CompressedPublicKey,
    // type of the transaction
    data: TransactionType,
    // fees in XELIS
    fee: u64,
    // nonce must be equal to the one on chain account
    // used to prevent replay attacks and have ordered transactions
    nonce: u64,
    // signature of this Transaction by the owner
    // signature: Signature
}

impl Serializer for TransferPayload {
    fn write(&self, writer: &mut Writer) {
        self.asset.write(writer);
        self.destination.write(writer);
        writer.write_bool(self.extra_data.is_some());
        if let Some(extra_data) = &self.extra_data {
            writer.write_u16(extra_data.len() as u16);
            writer.write_bytes(extra_data);
        }
        self.commitment.write(writer);
        self.sender_handle.write(writer);
        self.receiver_handle.write(writer);
    }

    fn read(reader: &mut Reader) -> Result<TransferPayload, ReaderError> {
        let asset = Hash::read(reader)?;
        let destination = CompressedPublicKey::read(reader)?;
        let has_extra_data = reader.read_bool()?;
        let extra_data = if has_extra_data {
            let extra_data_size = reader.read_u16()? as usize;
            if extra_data_size > EXTRA_DATA_LIMIT_SIZE {
                return Err(ReaderError::InvalidSize)
            }

            Some(reader.read_bytes(extra_data_size)?)
        } else {
            None
        };

        let commitment = CompressedCommitment::read(reader)?;
        let sender_handle = CompressedHandle::read(reader)?;
        let receiver_handle = CompressedHandle::read(reader)?;

        Ok(TransferPayload {
            asset,
            destination,
            extra_data,
            commitment,
            sender_handle,
            receiver_handle
        })
    }

    fn size(&self) -> usize {
        // + 1 for the bool
        let mut size = self.asset.size() + self.destination.size() + 1 + self.commitment.size() + self.sender_handle.size() + self.receiver_handle.size();
        if let Some(extra_data) = &self.extra_data {
            // + 2 for the size of the extra data
            size += 2 + extra_data.len();
        }
        size
    }
}

impl Serializer for BurnPayload {
    fn write(&self, writer: &mut Writer) {
        self.asset.write(writer);
        self.amount.write(writer);
    }

    fn read(reader: &mut Reader) -> Result<BurnPayload, ReaderError> {
        let asset = Hash::read(reader)?;
        let amount = reader.read_u64()?;
        Ok(BurnPayload {
            asset,
            amount
        })
    }

    fn size(&self) -> usize {
        self.asset.size() + self.amount.size()
    }
}

impl Serializer for TransactionType {
    fn write(&self, writer: &mut Writer) {
        match self {
            TransactionType::Burn(payload) => {
                writer.write_u8(0);
                payload.write(writer);
            }
            TransactionType::Transfers(txs) => {
                writer.write_u8(1);
                // max 255 txs per transaction
                let len: u8 = txs.len() as u8;
                writer.write_u8(len);
                for tx in txs {
                    tx.write(writer);
                }
            }
        };
    }

    fn read(reader: &mut Reader) -> Result<TransactionType, ReaderError> {
        Ok(match reader.read_u8()? {
            0 => {
                let payload = BurnPayload::read(reader)?;
                TransactionType::Burn(payload)
            },
            1 => {
                let txs_count = reader.read_u8()?;
                if txs_count == 0 || txs_count > MAX_TRANSFER_COUNT as u8 {
                    return Err(ReaderError::InvalidSize)
                }

                let mut txs = Vec::with_capacity(txs_count as usize);
                for _ in 0..txs_count {
                    txs.push(TransferPayload::read(reader)?);
                }
                TransactionType::Transfers(txs)
            },
            _ => {
                return Err(ReaderError::InvalidValue)
            }
        })
    }

    fn size(&self) -> usize {
        match self {
            TransactionType::Burn(payload) => {
                1 + payload.size()
            },
            TransactionType::Transfers(txs) => {
                let mut size = 1;
                for tx in txs {
                    size += tx.size();
                }
                size
            }
        }
    }
}

impl Transaction {
    pub fn new(source: CompressedPublicKey, data: TransactionType, fee: u64, nonce: u64, _signature: Signature) -> Self {
        Transaction {
            version: 0,
            source,
            data,
            fee,
            nonce,
            // signature
        }
    }

    pub fn get_version(&self) -> u8 {
        self.version
    }

    pub fn get_source(&self) -> &CompressedPublicKey {
        &self.source
    }

    pub fn get_data(&self) -> &TransactionType {
        &self.data
    }

    pub fn get_fee(&self) -> u64 {
        self.fee
    }

    pub fn get_nonce(&self) -> u64 {
        self.nonce
    }

    // // verify the validity of the signature
    // pub fn verify_signature(&self) -> bool {
    //     let bytes = self.to_bytes();
    //     let bytes = &bytes[0..bytes.len() - SIGNATURE_LENGTH]; // remove signature bytes for verification
    //     self.source.verify_signature(&hash(bytes), &self.signature)
    // }

    pub fn consume(self) -> (CompressedPublicKey, TransactionType) {
        (self.source, self.data)
    }
}

impl Serializer for Transaction {
    fn write(&self, writer: &mut Writer) {
        writer.write_u8(self.version);
        self.source.write(writer);
        self.data.write(writer);
        writer.write_u64(&self.fee);
        writer.write_u64(&self.nonce);
        // self.signature.write(writer);
    }

    fn read(reader: &mut Reader) -> Result<Transaction, ReaderError> {
        let version = reader.read_u8()?;
        // At this moment we only support version 0, so we check it here directly
        if version != 0 {
            debug!("Expected version 0 got version {version}");
            return Err(ReaderError::InvalidValue)
        }

        let source = CompressedPublicKey::read(reader)?;
        let data = TransactionType::read(reader)?;
        let fee = reader.read_u64()?;
        let nonce = reader.read_u64()?;
        // let signature = Signature::read(reader)?;

        Ok(Transaction {
            version,
            source,
            data,
            fee,
            nonce,
            // signature
        })
    }

    fn size(&self) -> usize {
        1 + self.source.size() + self.data.size() + self.fee.size() + self.nonce.size() // + self.signature.size()
    }
}

impl Hashable for Transaction {}

impl AsRef<Transaction> for Transaction {
    fn as_ref(&self) -> &Transaction {
        self
    }
}