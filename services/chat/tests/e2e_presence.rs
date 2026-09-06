//! Presence + room enter/leave e2e (design §10).

mod harness;

use std::time::Duration;

use bytes::Bytes;
use harness::*;
use kim_protocol::pkt::{
    Flag, PresencePush, PresenceStatus, RoomEnterReq, RoomEnterResp, RoomLeaveReq, Status,
};
use kim_protocol::{
    marshal, read, LogicPkt, Packet, CMD_PRESENCE, CMD_ROOM_ENTER, CMD_ROOM_LEAVE, INBOX_KIND_USER,
};

fn enter_pkt(seq: u32, dest: &str) -> LogicPkt {
    let mut pkt = LogicPkt::new(CMD_ROOM_ENTER, seq, Bytes::new());
    pkt.write_body(&RoomEnterReq {
        dest: dest.into(),
        kind: INBOX_KIND_USER,
    });
    pkt
}

fn leave_pkt(seq: u32, dest: &str) -> LogicPkt {
    let mut pkt = LogicPkt::new(CMD_ROOM_LEAVE, seq, Bytes::new());
    pkt.write_body(&RoomLeaveReq {
        dest: dest.into(),
        kind: INBOX_KIND_USER,
    });
    pkt
}

fn set_short_debounce() {
    // Must run before spawn_stack — PresenceHub reads env at construction.
    std::env::set_var("KIM_PRESENCE_OFFLINE_DEBOUNCE_MS", "250");
}

async fn expect_presence_push(client: &kim_ws::WsClient, account: &str, status: PresenceStatus) {
    let frame = timeout_read(client).await;
    match read(&frame.payload).expect("push frame") {
        Packet::Logic(p) => {
            assert_eq!(p.header.flag, Flag::Push as i32);
            assert_eq!(p.header.command, CMD_PRESENCE);
            let push: PresencePush = p.read_body().expect("presence body");
            assert_eq!(push.entries.len(), 1);
            assert_eq!(push.entries[0].account, account);
            assert_eq!(push.entries[0].status, status as i32);
            if status == PresenceStatus::PresenceOffline {
                assert!(push.entries[0].last_seen > 0);
            }
        }
        _ => panic!("expected presence push"),
    }
}

#[tokio::test]
async fn enter_requires_friendship() {
    set_short_debounce();
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (_bob, _) = login("bob", &url).await;

    alice
        .send(marshal(&Packet::Logic(enter_pkt(1, "bob"))))
        .await
        .expect("enter");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::NotFriends as i32),
        _ => panic!("expected logic"),
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn enter_snapshot_and_idempotent() {
    set_short_debounce();
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    for seq in [2u32, 3] {
        alice
            .send(marshal(&Packet::Logic(enter_pkt(seq, "bob"))))
            .await
            .expect("enter");
        let frame = timeout_read(&alice).await;
        match read(&frame.payload).expect("resp") {
            Packet::Logic(p) => {
                assert_eq!(p.header.status, Status::Success as i32);
                let resp: RoomEnterResp = p.read_body().expect("body");
                assert_eq!(resp.presence.len(), 1);
                assert_eq!(resp.presence[0].account, "bob");
                assert_eq!(
                    resp.presence[0].status,
                    PresenceStatus::PresenceOnline as i32
                );
            }
            _ => panic!("expected logic"),
        }
    }

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn presence_fanout_only_to_interested_viewer() {
    set_short_debounce();
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    let (carol, _) = login("carol", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;
    become_friends(&carol, &bob, "bob", "carol").await;

    alice
        .send(marshal(&Packet::Logic(enter_pkt(4, "bob"))))
        .await
        .expect("enter");
    let _ = timeout_read(&alice).await;

    bob.close().await.expect("bob close");
    expect_presence_push(&alice, "bob", PresenceStatus::PresenceOffline).await;
    timeout_no_packet(&carol, Duration::from_millis(400)).await;

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn leave_stops_fanout() {
    set_short_debounce();
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(enter_pkt(5, "bob"))))
        .await
        .expect("enter");
    let _ = timeout_read(&alice).await;

    alice
        .send(marshal(&Packet::Logic(leave_pkt(6, "bob"))))
        .await
        .expect("leave");
    let frame = timeout_read(&alice).await;
    match read(&frame.payload).expect("leave resp") {
        Packet::Logic(p) => assert_eq!(p.header.status, Status::Success as i32),
        _ => panic!("expected leave resp"),
    }

    bob.close().await.expect("bob close");
    timeout_no_packet(&alice, Duration::from_millis(600)).await;

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn debounce_reconnect_skips_offline() {
    set_short_debounce();
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(enter_pkt(7, "bob"))))
        .await
        .expect("enter");
    let _ = timeout_read(&alice).await;

    bob.close().await.expect("close");
    // Reconnect before debounce fires.
    let (bob2, _) = login("bob", &url).await;
    // Should get ONLINE (OFFLINE→ONLINE again) — first location after empty.
    expect_presence_push(&alice, "bob", PresenceStatus::PresenceOnline).await;
    // No OFFLINE should arrive after debounce window.
    timeout_no_packet(&alice, Duration::from_millis(500)).await;

    let _ = bob2;
    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn multi_device_stays_online_when_one_leaves() {
    set_short_debounce();
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob_a, _) = login_with_device("bob", &url, "web").await;
    let (_bob_b, _) = login_with_device("bob", &url, "web").await;
    become_friends(&alice, &bob_a, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(enter_pkt(8, "bob"))))
        .await
        .expect("enter");
    let _ = timeout_read(&alice).await;

    bob_a.close().await.expect("close one device");
    timeout_no_packet(&alice, Duration::from_millis(600)).await;

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn heartbeat_does_not_push_presence() {
    set_short_debounce();
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(enter_pkt(9, "bob"))))
        .await
        .expect("enter");
    let _ = timeout_read(&alice).await;

    // Idle with live connections (gateway heartbeats renew location only).
    timeout_no_packet(&alice, Duration::from_millis(800)).await;

    let _ = bob;
    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}

#[tokio::test]
async fn peer_online_push_after_enter() {
    set_short_debounce();
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let (alice, _) = login("alice", &url).await;
    let (mut bob, _) = login("bob", &url).await;
    become_friends(&alice, &bob, "bob", "alice").await;

    alice
        .send(marshal(&Packet::Logic(enter_pkt(10, "bob"))))
        .await
        .expect("enter");
    let _ = timeout_read(&alice).await;

    bob.close().await.expect("close");
    expect_presence_push(&alice, "bob", PresenceStatus::PresenceOffline).await;

    let (_bob2, _) = login("bob", &url).await;
    expect_presence_push(&alice, "bob", PresenceStatus::PresenceOnline).await;

    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
