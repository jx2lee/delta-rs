# 수정 가이드: 느린 bin 하나가 완료된 bin 의 커밋을 막음 (#4749)

난이도: ★★☆☆☆ (Option A 는 1줄, Option B 는 async 스트림 이해 필요)

## 1. 문제를 한 줄로

compaction 은 결과를 **순서대로** 받는 `buffered` 를 쓰고, 커밋 여부는 결과가 하나 나올 때만 검사합니다.
그래서 맨 앞 bin 이 느리면 뒤에서 끝난 bin 들이 커밋되지 못한 채 메모리에만 쌓입니다.

## 2. 코드 읽기

`crates/core/src/operations/optimize.rs`

```text
 845-866  Compact: bins 를 스트림으로 만들고 .buffered(max_concurrent_tasks)
 868-908  ZOrder : 같은 구조인데 .buffer_unordered(max_concurrent_tasks)
 927-975  loop { stream.next().await → actions 에 누적 → mature(간격 경과) || end 면 commit }
```

Rust/async 개념 메모:
- `futures::StreamExt::buffered(n)`: 최대 n 개의 future 를 동시에 돌리되 **입력 순서대로** 결과를 내보냄.
- `buffer_unordered(n)`: 동시에 n 개, **끝나는 순서대로** 결과를 내보냄.
- 두 메서드 모두 이미 import 되어 있고 타입도 같아서 바꿔 끼우면 그대로 컴파일됩니다.

## 3. 먼저 이슈에 질문 댓글

이 `buffered` 는 `86d97cca fix: preserve compact locality by default` 에서 의도적으로 바뀐 것입니다.
이슈 작성자도 "ordering 이 의도된 것이냐" 고 묻고 답이 없습니다. PR 전에 이슈에 아래처럼 짧게 남기세요.

> I traced the ordered `buffered` back to 86d97cca (preserve compact locality). As far as I can see the
> locality guarantee comes from how bins are planned, not from the order results are yielded, since all
> actions of a bin are committed together either way. If that's right I'd like to open a PR switching the
> compaction arm to `buffer_unordered` like z-order. Happy to do the timer-based flush instead if ordering
> must stay.

## 4. Option A: 한 줄 수정

```rust
-                    .buffered(max_concurrent_tasks)
+                    .buffer_unordered(max_concurrent_tasks)
```

그리고 `cargo test -p deltalake-core --features datafusion --test it_datafusion optimize` 를 돌려서
locality 관련 테스트(`test_ordered_compact_bins_*`, `command_optimize.rs` 의 locality 테스트)가 통과하는지 확인합니다.
통과하면 "커밋 순서는 locality 와 무관하다" 는 근거가 됩니다.

## 5. Option B: 타이머로도 커밋

순서를 유지해야 한다는 답이 오면, 커밋 루프에서 "스트림이 멈춰 있어도 간격이 지나면 flush" 하도록 합니다.

```rust
let mut tick = min_commit_interval.map(|i| tokio::time::interval(i));
loop {
    let next = match tick.as_mut() {
        Some(t) => tokio::select! {
            n = stream.next() => n.transpose()?,
            _ = t.tick() => { /* 스트림은 안 움직였지만 flush 검사만 하고 계속 */ continue_if_nothing_buffered }
        },
        None => stream.next().await.transpose()?,
    };
    ...
}
```

이 방식은 `select!` 로 `stream.next()` 가 취소될 때 안전한지(StreamExt::next 는 cancel-safe 합니다) 를 PR 에 적어야 합니다.
처음이라면 Option A 로 시작하는 것을 권합니다.

## 6. 테스트 작성 팁 (Option A 검증)

`command_optimize.rs` 의 `OperationTrackingLogStore` 는 로그스토어 호출을 기록하는 래퍼입니다.
같은 방식으로 **object store** 의 `put` 을 가로채 특정 파일 이름(첫 bin 의 출력)만 `tokio::time::sleep` 시키면
"첫 bin 이 느린" 상황을 만들 수 있습니다. 그 상태에서

```rust
.with_min_commit_interval(Duration::from_millis(1))
.with_max_concurrent_tasks(3)
```

로 compaction 을 돌리고, 느린 bin 이 끝나기 **전에** 커밋이 1개 이상 있는지(`table.version()` 증가) 확인합니다.
복잡하면 테스트 없이 올리고 PR 본문에 재현 조건을 서술해도 됩니다.

## 7. 체크리스트

```bash
cargo fmt --all
cargo clippy -p deltalake-core --features datafusion --tests
git commit -s -m "fix: yield finished compaction bins out of order so they can be committed"
```
