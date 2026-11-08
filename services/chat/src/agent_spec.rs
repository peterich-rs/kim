use bytes::Bytes;
use kim_protocol::pkt::{AgentSpecSyncResp, AgentSpecUpsertReq, AgentSpecUpsertResp, Status};
use kim_router::{Context, RouterError};
use kim_server_data::agent_spec::{
    account_from_pb, account_to_pb, record_from_pb, record_to_pb, AgentSpecError, AgentSpecStore,
};
use tracing::warn;

fn status_of(err: &AgentSpecError) -> Status {
    match err {
        AgentSpecError::Invalid => Status::InvalidPacketBody,
        AgentSpecError::NotOwner => Status::NotBotOwner,
        AgentSpecError::Backend(_) => Status::SystemException,
    }
}

fn owner_or_deny(ctx: &Context) -> Result<String, Status> {
    let dest = ctx.header().dest.as_str();
    if !dest.is_empty() && dest != ctx.session().account {
        return Err(Status::NotBotOwner);
    }
    Ok(ctx.session().account.clone())
}

pub async fn do_agent_spec_sync(
    ctx: Context,
    store: &dyn AgentSpecStore,
) -> Result<(), RouterError> {
    let owner = match owner_or_deny(&ctx) {
        Ok(o) => o,
        Err(status) => {
            ctx.resp_bytes(status, Bytes::new()).await?;
            return Ok(());
        }
    };
    let records = match store.list(&ctx.session().app, &owner).await {
        Ok(rows) => rows,
        Err(err) => {
            warn!(%err, "agent spec sync failed");
            ctx.resp_with_error(status_of(&err), &err).await?;
            return Ok(());
        }
    };
    let accounts = match store.list_accounts(&ctx.session().app, &owner).await {
        Ok(rows) => rows,
        Err(err) => {
            warn!(%err, "agent account sync failed");
            ctx.resp_with_error(status_of(&err), &err).await?;
            return Ok(());
        }
    };
    ctx.resp(
        Status::Success,
        Some(&AgentSpecSyncResp {
            records: records.iter().map(record_to_pb).collect(),
            accounts: accounts.iter().map(account_to_pb).collect(),
        }),
    )
    .await?;
    Ok(())
}

pub async fn do_agent_spec_upsert(
    ctx: Context,
    store: &dyn AgentSpecStore,
) -> Result<(), RouterError> {
    let owner = match owner_or_deny(&ctx) {
        Ok(o) => o,
        Err(status) => {
            ctx.resp_bytes(status, Bytes::new()).await?;
            return Ok(());
        }
    };
    let req = match ctx.read_body::<AgentSpecUpsertReq>() {
        Ok(r) => r,
        Err(err) => {
            ctx.resp_with_error(Status::InvalidPacketBody, &err).await?;
            return Ok(());
        }
    };
    if req.record.is_none() && req.account.is_none() {
        ctx.resp_bytes(Status::InvalidPacketBody, Bytes::new())
            .await?;
        return Ok(());
    }
    if let Some(acc) = req.account {
        let rec = account_from_pb(acc);
        if rec.id.is_empty() {
            ctx.resp_bytes(Status::InvalidPacketBody, Bytes::new())
                .await?;
            return Ok(());
        }
        if let Err(err) = store.upsert_account(&ctx.session().app, &owner, rec).await {
            warn!(%err, "agent account upsert failed");
            ctx.resp_with_error(status_of(&err), &err).await?;
            return Ok(());
        }
    }
    let Some(pb) = req.record else {
        ctx.resp(Status::Success, Some(&AgentSpecUpsertResp { record: None }))
            .await?;
        return Ok(());
    };
    let rec = record_from_pb(pb);
    if rec.profile_id.is_empty() || rec.spec.is_empty() {
        ctx.resp_bytes(Status::InvalidPacketBody, Bytes::new())
            .await?;
        return Ok(());
    }
    match store.upsert(&ctx.session().app, &owner, rec).await {
        Ok(stored) => {
            ctx.resp(
                Status::Success,
                Some(&AgentSpecUpsertResp {
                    record: Some(record_to_pb(&stored)),
                }),
            )
            .await?;
        }
        Err(err) => {
            warn!(%err, "agent spec upsert failed");
            ctx.resp_with_error(status_of(&err), &err).await?;
        }
    }
    Ok(())
}
