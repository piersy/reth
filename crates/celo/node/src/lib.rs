//! Celo node implementation for reth.
//!
//! This crate provides the Celo node type that integrates all Celo-specific
//! components with reth's modular node builder system.
//!
//! # Architecture
//!
//! The Celo node extends reth's Ethereum node with:
//! - [`CeloChainSpec`] for Celo chain configuration
//! - [`CeloEvmConfig`] for Celo-specific EVM behavior (via celo-revm)
//! - [`CeloBeaconConsensus`] for Celo consensus validation
//! - Standard Ethereum payload builder (fee currency handling is at EVM level)
//! - Standard Ethereum transaction pool (with Celo validation extensions)

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]

use reth_celo_chainspec::CeloChainSpec;
use reth_celo_consensus::CeloBeaconConsensus;
use reth_celo_evm::CeloEvmConfig;
use reth_celo_forks::Hardforks;

use alloy_eips::{eip7840::BlobParams, merge::EPOCH_SLOTS};
use alloy_network::Ethereum;
use alloy_rpc_types_engine::ExecutionData;
use reth_chainspec::{EthChainSpec, EthereumHardforks};
use reth_engine_primitives::EngineTypes;
use reth_ethereum_engine_primitives::{
    EthBuiltPayload, EthPayloadAttributes, EthPayloadBuilderAttributes,
};
use reth_ethereum_payload_builder::EthereumBuilderConfig;
use reth_ethereum_primitives::{EthPrimitives, TransactionSigned};
use reth_evm::{ConfigureEvm, EvmFactory, EvmFactoryFor, NextBlockEnvAttributes};
use reth_network::{primitives::BasicNetworkPrimitives, NetworkHandle, PeersInfo};
use reth_node_api::{
    AddOnsContext, FullNodeComponents, HeaderTy, NodeAddOns, NodePrimitives, PrimitivesTy, TxTy,
};
use reth_node_builder::{
    components::{
        BasicPayloadServiceBuilder, ComponentsBuilder, ConsensusBuilder, ExecutorBuilder,
        NetworkBuilder, PayloadBuilderBuilder, PoolBuilder, TxPoolBuilder,
    },
    node::{FullNodeTypes, NodeTypes},
    rpc::{
        BasicEngineApiBuilder, BasicEngineValidatorBuilder, EngineApiBuilder,
        EngineValidatorAddOn, EngineValidatorBuilder, EthApiBuilder, EthApiCtx, Identity,
        PayloadValidatorBuilder, RethRpcAddOns, RpcAddOns, RpcHandle,
    },
    BuilderContext, Node, NodeAdapter, PayloadBuilderConfig, PayloadTypes,
};
use reth_provider::{providers::ProviderFactoryBuilder, EthStorage};
use reth_rpc::{
    eth::core::{EthApiFor, EthRpcConverterFor},
    ValidationApi,
};
use reth_rpc_api::servers::BlockSubmissionValidationApiServer;
use reth_rpc_builder::{config::RethRpcServerConfig, middleware::RethRpcMiddleware};
use reth_rpc_eth_api::{
    helpers::pending_block::BuildPendingEnv, RpcConvert, RpcTypes, SignableTxRequest,
};
use reth_rpc_eth_types::{error::FromEvmError, EthApiError};
use reth_rpc_server_types::RethRpcModule;
use reth_tracing::tracing::info;
use reth_transaction_pool::{
    blobstore::DiskFileBlobStore, EthTransactionPool, PoolPooledTx, PoolTransaction,
    TransactionPool, TransactionValidationTaskExecutor,
};
use revm::context::TxEnv;
use std::{marker::PhantomData, sync::Arc, time::SystemTime};

pub use reth_node_ethereum::EthereumEngineValidator;

/// Type configuration for a Celo node.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct CeloNode;

/// Engine types for Celo (same as Ethereum).
pub type CeloEngineTypes = reth_ethereum_engine_primitives::EthEngineTypes;

impl NodeTypes for CeloNode {
    type Primitives = EthPrimitives;
    type ChainSpec = CeloChainSpec;
    type Storage = EthStorage;
    type Payload = CeloEngineTypes;
}

impl CeloNode {
    /// Returns a [`ComponentsBuilder`] configured for a Celo node.
    pub fn components<Node>() -> ComponentsBuilder<
        Node,
        CeloPoolBuilder,
        BasicPayloadServiceBuilder<CeloPayloadBuilder>,
        CeloNetworkBuilder,
        CeloExecutorBuilder,
        CeloConsensusBuilder,
    >
    where
        Node: FullNodeTypes<
            Types: NodeTypes<
                ChainSpec = CeloChainSpec,
                Primitives = EthPrimitives,
            >,
        >,
        <Node::Types as NodeTypes>::Payload: PayloadTypes<
            BuiltPayload = EthBuiltPayload,
            PayloadAttributes = EthPayloadAttributes,
            PayloadBuilderAttributes = EthPayloadBuilderAttributes,
        >,
    {
        ComponentsBuilder::default()
            .node_types::<Node>()
            .pool(CeloPoolBuilder::default())
            .executor(CeloExecutorBuilder::default())
            .payload(BasicPayloadServiceBuilder::default())
            .network(CeloNetworkBuilder::default())
            .consensus(CeloConsensusBuilder::default())
    }

    /// Instantiates the [`ProviderFactoryBuilder`] for a Celo node.
    pub fn provider_factory_builder() -> ProviderFactoryBuilder<Self> {
        ProviderFactoryBuilder::default()
    }
}

impl<N> Node<N> for CeloNode
where
    N: FullNodeTypes<Types = Self>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        CeloPoolBuilder,
        BasicPayloadServiceBuilder<CeloPayloadBuilder>,
        CeloNetworkBuilder,
        CeloExecutorBuilder,
        CeloConsensusBuilder,
    >;

    type AddOns = CeloAddOns<
        NodeAdapter<N>,
        CeloEthApiBuilder,
        CeloEngineValidatorBuilder,
    >;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        Self::components()
    }

    fn add_ons(&self) -> Self::AddOns {
        CeloAddOns::default()
    }
}

/// Celo payload builder.
///
/// This is a unit struct that implements [`PayloadBuilderBuilder`] to construct
/// the actual payload builder with the correct type parameters at build time.
#[derive(Clone, Default, Debug)]
#[non_exhaustive]
pub struct CeloPayloadBuilder;

impl<Types, Node, Pool, Evm> PayloadBuilderBuilder<Node, Pool, Evm> for CeloPayloadBuilder
where
    Types: NodeTypes<ChainSpec: EthereumHardforks, Primitives = EthPrimitives>,
    Node: FullNodeTypes<Types = Types>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TxTy<Node::Types>>>
        + Unpin
        + 'static,
    Evm: ConfigureEvm<
            Primitives = PrimitivesTy<Types>,
            NextBlockEnvCtx = NextBlockEnvAttributes,
        > + 'static,
    Types::Payload: PayloadTypes<
        BuiltPayload = EthBuiltPayload,
        PayloadAttributes = EthPayloadAttributes,
        PayloadBuilderAttributes = EthPayloadBuilderAttributes,
    >,
{
    type PayloadBuilder =
        reth_ethereum_payload_builder::EthereumPayloadBuilder<Pool, Node::Provider, Evm>;

    async fn build_payload_builder(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
        evm_config: Evm,
    ) -> eyre::Result<Self::PayloadBuilder> {
        let conf = ctx.payload_builder_config();
        let chain = ctx.chain_spec().chain();
        let gas_limit = conf.gas_limit_for(chain);

        Ok(reth_ethereum_payload_builder::EthereumPayloadBuilder::new(
            ctx.provider().clone(),
            pool,
            evm_config,
            EthereumBuilderConfig::new()
                .with_gas_limit(gas_limit)
                .with_max_blobs_per_block(conf.max_blobs_per_block())
                .with_extra_data(conf.extra_data_bytes()),
        ))
    }
}

/// Celo EVM executor builder.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct CeloExecutorBuilder;

impl<Types, Node> ExecutorBuilder<Node> for CeloExecutorBuilder
where
    Types: NodeTypes<
        ChainSpec = CeloChainSpec,
        Primitives = EthPrimitives,
    >,
    Node: FullNodeTypes<Types = Types>,
{
    type EVM = CeloEvmConfig;

    async fn build_evm(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::EVM> {
        Ok(CeloEvmConfig::new(ctx.chain_spec()))
    }
}

/// Celo transaction pool builder.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct CeloPoolBuilder;

impl<Types, Node, Evm> PoolBuilder<Node, Evm> for CeloPoolBuilder
where
    Types: NodeTypes<
        ChainSpec: EthereumHardforks,
        Primitives: NodePrimitives<SignedTx = TransactionSigned>,
    >,
    Node: FullNodeTypes<Types = Types>,
    Evm: ConfigureEvm<Primitives = PrimitivesTy<Types>> + Clone + 'static,
{
    type Pool = EthTransactionPool<Node::Provider, DiskFileBlobStore, Evm>;

    async fn build_pool(
        self,
        ctx: &BuilderContext<Node>,
        evm_config: Evm,
    ) -> eyre::Result<Self::Pool> {
        let pool_config = ctx.pool_config();

        let blobs_disabled = ctx.config().txpool.disable_blobs_support
            || ctx.config().txpool.blobpool_max_count == 0;

        let blob_cache_size = if let Some(blob_cache_size) = pool_config.blob_cache_size {
            Some(blob_cache_size)
        } else {
            let current_timestamp =
                SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
            let blob_params = ctx
                .chain_spec()
                .blob_params_at_timestamp(current_timestamp)
                .unwrap_or_else(BlobParams::cancun);
            Some((blob_params.target_blob_count * EPOCH_SLOTS * 2) as u32)
        };

        let blob_store =
            reth_node_builder::components::create_blob_store_with_cache(ctx, blob_cache_size)?;

        let validator =
            TransactionValidationTaskExecutor::eth_builder(ctx.provider().clone(), evm_config)
                .set_eip4844(!blobs_disabled)
                .kzg_settings(ctx.kzg_settings()?)
                .with_max_tx_input_bytes(ctx.config().txpool.max_tx_input_bytes)
                .with_local_transactions_config(pool_config.local_transactions_config.clone())
                .set_tx_fee_cap(ctx.config().rpc.rpc_tx_fee_cap)
                .with_max_tx_gas_limit(ctx.config().txpool.max_tx_gas_limit)
                .with_minimum_priority_fee(ctx.config().txpool.minimum_priority_fee)
                .with_additional_tasks(ctx.config().txpool.additional_validation_tasks)
                .build_with_tasks(ctx.task_executor().clone(), blob_store.clone());

        let transaction_pool = TxPoolBuilder::new(ctx)
            .with_validator(validator)
            .build_and_spawn_maintenance_task(blob_store, pool_config)?;

        info!(target: "reth::cli", "Celo transaction pool initialized");

        Ok(transaction_pool)
    }
}

/// Celo network builder.
#[derive(Debug, Default, Clone, Copy)]
pub struct CeloNetworkBuilder;

impl<Node, Pool> NetworkBuilder<Node, Pool> for CeloNetworkBuilder
where
    Node: FullNodeTypes<Types: NodeTypes<ChainSpec: Hardforks>>,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TxTy<Node::Types>>>
        + Unpin
        + 'static,
{
    type Network =
        NetworkHandle<BasicNetworkPrimitives<PrimitivesTy<Node::Types>, PoolPooledTx<Pool>>>;

    async fn build_network(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<Self::Network> {
        let network = ctx.network_builder().await?;
        let handle = ctx.start_network(network, pool);
        info!(target: "reth::cli", enode=%handle.local_node_record(), "Celo P2P networking initialized");
        Ok(handle)
    }
}

/// Celo consensus builder.
#[derive(Debug, Default, Clone, Copy)]
pub struct CeloConsensusBuilder;

impl<Node> ConsensusBuilder<Node> for CeloConsensusBuilder
where
    Node: FullNodeTypes<
        Types: NodeTypes<ChainSpec = CeloChainSpec, Primitives = EthPrimitives>,
    >,
{
    type Consensus = Arc<CeloBeaconConsensus>;

    async fn build_consensus(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Consensus> {
        Ok(Arc::new(CeloBeaconConsensus::new(ctx.chain_spec())))
    }
}

/// Builds the Celo Eth API.
#[derive(Debug)]
pub struct CeloEthApiBuilder<NetworkT = Ethereum>(PhantomData<NetworkT>);

impl<NetworkT> Default for CeloEthApiBuilder<NetworkT> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<N, NetworkT> EthApiBuilder<N> for CeloEthApiBuilder<NetworkT>
where
    N: FullNodeComponents<
        Types: NodeTypes<ChainSpec: Hardforks + EthereumHardforks>,
        Evm: ConfigureEvm<NextBlockEnvCtx: BuildPendingEnv<HeaderTy<N::Types>>>,
    >,
    NetworkT: RpcTypes<TransactionRequest: SignableTxRequest<TxTy<N::Types>>>,
    EthRpcConverterFor<N, NetworkT>: RpcConvert<
        Primitives = PrimitivesTy<N::Types>,
        Error = EthApiError,
        Network = NetworkT,
        Evm = N::Evm,
    >,
    EthApiError: FromEvmError<N::Evm>,
{
    type EthApi = EthApiFor<N, NetworkT>;

    async fn build_eth_api(self, ctx: EthApiCtx<'_, N>) -> eyre::Result<Self::EthApi> {
        Ok(ctx.eth_api_builder().map_converter(|r| r.with_network()).build())
    }
}

/// Celo engine validator builder.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct CeloEngineValidatorBuilder;

impl<Node, Types> PayloadValidatorBuilder<Node> for CeloEngineValidatorBuilder
where
    Types: NodeTypes<
        ChainSpec: Hardforks + EthereumHardforks + EthChainSpec + Clone + 'static,
        Payload: EngineTypes<ExecutionData = ExecutionData>
            + PayloadTypes<PayloadAttributes = EthPayloadAttributes>,
        Primitives = EthPrimitives,
    >,
    Node: FullNodeComponents<Types = Types>,
{
    type Validator = EthereumEngineValidator<Types::ChainSpec>;

    async fn build(self, ctx: &AddOnsContext<'_, Node>) -> eyre::Result<Self::Validator> {
        Ok(EthereumEngineValidator::new(ctx.config.chain.clone()))
    }
}

/// Add-ons for Celo.
#[derive(Debug)]
pub struct CeloAddOns<
    N: FullNodeComponents,
    EthB: EthApiBuilder<N>,
    PVB,
    EB = BasicEngineApiBuilder<PVB>,
    EVB = BasicEngineValidatorBuilder<PVB>,
    RpcMiddleware = Identity,
> {
    inner: RpcAddOns<N, EthB, PVB, EB, EVB, RpcMiddleware>,
}

impl<N, EthB, PVB, EB, EVB, RpcMiddleware> CeloAddOns<N, EthB, PVB, EB, EVB, RpcMiddleware>
where
    N: FullNodeComponents,
    EthB: EthApiBuilder<N>,
{
    /// Creates a new instance from the inner `RpcAddOns`.
    pub const fn new(inner: RpcAddOns<N, EthB, PVB, EB, EVB, RpcMiddleware>) -> Self {
        Self { inner }
    }
}

impl<N> Default for CeloAddOns<N, CeloEthApiBuilder, CeloEngineValidatorBuilder>
where
    N: FullNodeComponents<
        Types: NodeTypes<
            ChainSpec: EthereumHardforks + EthChainSpec + Clone + 'static,
            Payload: EngineTypes<ExecutionData = ExecutionData>
                + PayloadTypes<PayloadAttributes = EthPayloadAttributes>,
            Primitives = EthPrimitives,
        >,
    >,
    CeloEthApiBuilder: EthApiBuilder<N>,
{
    fn default() -> Self {
        Self::new(RpcAddOns::new(
            CeloEthApiBuilder::default(),
            CeloEngineValidatorBuilder::default(),
            BasicEngineApiBuilder::default(),
            BasicEngineValidatorBuilder::default(),
            Default::default(),
        ))
    }
}

impl<N, EthB, PVB, EB, EVB, RpcMiddleware> NodeAddOns<N>
    for CeloAddOns<N, EthB, PVB, EB, EVB, RpcMiddleware>
where
    N: FullNodeComponents<
        Types: NodeTypes<
            ChainSpec: Hardforks + EthereumHardforks,
            Primitives = EthPrimitives,
            Payload: EngineTypes<ExecutionData = ExecutionData>,
        >,
        Evm: ConfigureEvm<NextBlockEnvCtx = NextBlockEnvAttributes>,
    >,
    EthB: EthApiBuilder<N>,
    PVB: Send,
    EB: EngineApiBuilder<N>,
    EVB: EngineValidatorBuilder<N>,
    EthApiError: FromEvmError<N::Evm>,
    EvmFactoryFor<N::Evm>: EvmFactory<Tx = TxEnv>,
    RpcMiddleware: RethRpcMiddleware,
{
    type Handle = RpcHandle<N, EthB::EthApi>;

    async fn launch_add_ons(
        self,
        ctx: reth_node_api::AddOnsContext<'_, N>,
    ) -> eyre::Result<Self::Handle> {
        let validation_api = ValidationApi::<_, _, <N::Types as NodeTypes>::Payload>::new(
            ctx.node.provider().clone(),
            Arc::new(ctx.node.consensus().clone()),
            ctx.node.evm_config().clone(),
            ctx.config.rpc.flashbots_config(),
            ctx.node.task_executor().clone(),
            Arc::new(EthereumEngineValidator::new(ctx.config.chain.clone())),
        );

        self.inner
            .launch_add_ons_with(ctx, move |container| {
                container.modules.merge_if_module_configured(
                    RethRpcModule::Flashbots,
                    validation_api.into_rpc(),
                )?;
                Ok(())
            })
            .await
    }
}

impl<N, EthB, PVB, EB, EVB, RpcMiddleware> RethRpcAddOns<N>
    for CeloAddOns<N, EthB, PVB, EB, EVB, RpcMiddleware>
where
    N: FullNodeComponents<
        Types: NodeTypes<
            ChainSpec: Hardforks + EthereumHardforks,
            Primitives = EthPrimitives,
            Payload: EngineTypes<ExecutionData = ExecutionData>,
        >,
        Evm: ConfigureEvm<NextBlockEnvCtx = NextBlockEnvAttributes>,
    >,
    EthB: EthApiBuilder<N>,
    PVB: PayloadValidatorBuilder<N>,
    EB: EngineApiBuilder<N>,
    EVB: EngineValidatorBuilder<N>,
    EthApiError: FromEvmError<N::Evm>,
    EvmFactoryFor<N::Evm>: EvmFactory<Tx = TxEnv>,
    RpcMiddleware: RethRpcMiddleware,
{
    type EthApi = EthB::EthApi;

    fn hooks_mut(&mut self) -> &mut reth_node_builder::rpc::RpcHooks<N, Self::EthApi> {
        self.inner.hooks_mut()
    }
}

impl<N, EthB, PVB, EB, EVB, RpcMiddleware> EngineValidatorAddOn<N>
    for CeloAddOns<N, EthB, PVB, EB, EVB, RpcMiddleware>
where
    N: FullNodeComponents<
        Types: NodeTypes<
            ChainSpec: EthChainSpec + EthereumHardforks,
            Primitives = EthPrimitives,
            Payload: EngineTypes<ExecutionData = ExecutionData>,
        >,
        Evm: ConfigureEvm<NextBlockEnvCtx = NextBlockEnvAttributes>,
    >,
    EthB: EthApiBuilder<N>,
    PVB: Send,
    EB: EngineApiBuilder<N>,
    EVB: EngineValidatorBuilder<N>,
    EthApiError: FromEvmError<N::Evm>,
    EvmFactoryFor<N::Evm>: EvmFactory<Tx = TxEnv>,
    RpcMiddleware: Send,
{
    type ValidatorBuilder = EVB;

    fn engine_validator_builder(&self) -> Self::ValidatorBuilder {
        self.inner.engine_validator_builder()
    }
}
