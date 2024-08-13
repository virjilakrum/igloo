use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Read,
    sync::{Arc, RwLock},
};

use solana_sdk::{
    account::{AccountSharedData, WritableAccount},
    instruction::AccountMeta,
    pubkey::Pubkey,
    signature::Signature,
};
use solana_svm::{
    account_loader::{CheckedTransactionDetails, TransactionCheckResult},
    transaction_processing_callback::TransactionProcessingCallback,
    transaction_processor::{
        ExecutionRecordingConfig, LoadAndExecuteSanitizedTransactionsOutput,
        TransactionBatchProcessor, TransactionProcessingConfig,
    },
};

use crate::{
    bank::{BankInfo, BankOperations},
    builtin::register_builtins,
    env::create_executable_environment,
    mock::fork_graph::MockForkGraph,
    prelude::*,
    transaction::builder::SanitizedTransactionBuilder,
};

pub struct Settings {
    pub fee_payer_balance: u64,
}

pub struct ExecutionAccounts {
    pub fee_payer: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub signatures: HashMap<Pubkey, Signature>,
}

#[derive(Default)]
pub struct SimpleBuilder<B: TransactionProcessingCallback + BankOperations + Default> {
    bank: B,
    settings: Settings,
    tx_builder: SanitizedTransactionBuilder,

    program_path: Option<String>,
    program_buffer: Option<Vec<u8>>,
    calldata: Vec<u8>,
    accounts: Vec<(AccountMeta, Option<AccountSharedData>)>,
    v0_message: bool,

    check_result: Option<TransactionCheckResult>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fee_payer_balance: 80000,
        }
    }
}

impl<B> SimpleBuilder<B>
where
    B: TransactionProcessingCallback + BankOperations + BankInfo + Default,
{
    pub fn build(&mut self) -> Result<LoadAndExecuteSanitizedTransactionsOutput> {
        let buffer = self.read_program()?;
        let program_id = self.bank.deploy_program(buffer);

        let accounts = self.prepare_accounts();
        self.tx_builder.create_instruction(
            program_id,
            accounts.accounts,
            accounts.signatures,
            self.calldata.clone(),
        );

        let sanitized_transaction = self.tx_builder.build(
            self.bank.last_blockhash(),
            (accounts.fee_payer, Signature::new_unique()),
            self.v0_message,
        )?;
        let check_result = self.get_checked_tx_details();

        let batch_processor = TransactionBatchProcessor::<MockForkGraph>::new(
            self.bank.execution_slot(),
            self.bank.execution_epoch(),
            HashSet::new(),
        );
        let fork_graph = Arc::new(RwLock::new(MockForkGraph {}));
        create_executable_environment(
            fork_graph.clone(),
            &mut batch_processor.program_cache.write().unwrap(),
        );

        self.bank.set_clock();
        batch_processor.fill_missing_sysvar_cache_entries(&self.bank);

        register_builtins(&self.bank, &batch_processor);

        let processing_config = self.get_processing_config();
        Ok(batch_processor.load_and_execute_sanitized_transactions(
            &self.bank,
            &[sanitized_transaction],
            vec![check_result],
            &Default::default(),
            &processing_config,
        ))
    }

    pub fn mock_bank(&self) -> &B {
        &self.bank
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn program_path(&mut self, path: Option<String>) -> &mut Self {
        self.program_path = path;
        self
    }

    pub fn program_buffer(&mut self, buffer: Option<Vec<u8>>) -> &mut Self {
        self.program_buffer = buffer;
        self
    }

    pub fn calldata(&mut self, calldata: Vec<u8>) -> &mut Self {
        self.calldata = calldata;
        self
    }

    pub fn v0_message(&mut self, value: bool) -> &mut Self {
        self.v0_message = value;
        self
    }

    pub fn account(&mut self, meta: AccountMeta, account: Option<AccountSharedData>) -> &mut Self {
        self.accounts.push((meta, account));
        self
    }

    pub fn account_with_balance(
        &mut self,
        pubkey: Pubkey,
        lamports: Option<u64>,
        is_signer: bool,
        is_writable: bool,
    ) -> &mut Self {
        let account = if let Some(lamports) = lamports {
            let mut account = AccountSharedData::default();
            account.set_lamports(lamports);
            Some(account)
        } else {
            None
        };
        self.account(
            AccountMeta {
                pubkey,
                is_signer,
                is_writable,
            },
            account,
        )
    }

    pub fn check_result(&mut self, result: TransactionCheckResult) -> &mut Self {
        self.check_result = Some(result);
        self
    }

    fn prepare_accounts(&mut self) -> ExecutionAccounts {
        let mut accounts = vec![];
        let mut signatures = HashMap::new();
        for (meta, account) in self.accounts.iter() {
            if let Some(account) = account {
                self.bank.insert_account(meta.pubkey, account.clone());
            }

            accounts.push(meta.clone());

            if meta.is_signer {
                signatures.insert(meta.pubkey, Signature::new_unique());
            }
        }

        ExecutionAccounts {
            fee_payer: self.create_fee_payer(),
            accounts,
            signatures,
        }
    }

    fn get_checked_tx_details(&self) -> TransactionCheckResult {
        self.check_result
            .clone()
            .unwrap_or(Ok(CheckedTransactionDetails {
                nonce: None,
                lamports_per_signature: 20,
            }))
    }

    fn create_fee_payer(&mut self) -> Pubkey {
        let fee_payer = Pubkey::new_unique();
        let mut account_data = AccountSharedData::default();
        account_data.set_lamports(self.settings.fee_payer_balance);
        self.bank.insert_account(fee_payer, account_data);
        fee_payer
    }

    fn read_program(&self) -> Result<Vec<u8>> {
        if self.program_buffer.is_some() && self.program_path.is_some() {
            return Err(Error::BuilderError(
                "Both program buffer and path are set".into(),
            ));
        }

        if let Some(buffer) = self.program_buffer.clone() {
            return Ok(buffer);
        } else if let Some(path) = self.program_path.clone() {
            return self.read_file(&path);
        }

        Err(Error::BuilderError("Program not found".into()))
    }

    fn read_file(&self, dir: &str) -> Result<Vec<u8>> {
        let mut file = File::open(dir)?;
        let metadata = fs::metadata(dir)?;
        let mut buffer = vec![0; metadata.len() as usize];
        file.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    fn get_processing_config(&self) -> TransactionProcessingConfig {
        TransactionProcessingConfig {
            recording_config: ExecutionRecordingConfig {
                enable_log_recording: true,
                enable_return_data_recording: true,
                enable_cpi_recording: false,
            },
            ..Default::default()
        }
    }
}