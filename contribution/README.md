# delta-rs 기여 후보 (2026-10-13, main `b597cf23`)

조건: GitHub 에 등록된 **open issue** 이고(6번만 예외: 직접 찾은 버그라 이슈부터 올려야 함), 링크된 open/merged PR 이 없으며, 소스에서 원인을 확인하고 재현한 것.
각 폴더에 `issue.md`(이슈 요약·재현 결과), `pr.md`(브랜치·제목·본문 초안), `guide.md`(수정 방법, Rust 초심자용) 가 있습니다.

| # | 폴더 | 이슈 | 난이도 | 재현 방식 | 비고 |
|---|---|---|---|---|---|
| 2 | `uniform-table-properties-rejected` | #3299 | ★★ | in-memory create 로 에러 재현 | enum variant 만. writer feature 는 delta-rs 미지원이라 켜면 안 됨, 이슈에 방향 질문 먼저 |
| 3 | `zorder-dotted-column-names` | #4363 | ★★ | in-memory z-order 로 에러 재현 | kernel `ColumnName` 파서 사용. 이전 `take` 있음 |
| 4 | `compaction-ordered-buffer-delays-commits` | #4749 | ★★ | 코드·git 이력 대조 | 1줄 fix 가능하나 먼저 이슈에 방향 질문 |
| 5 | `nested-schema-override-widened-in-parquet-read` | #4678 | ★★★★ | `plan.rs` 단위 테스트 실패 확인 | 닫힌 PR 2개의 리뷰 코멘트 반영 필요 |
| 6 | `optimize-tilde-partition-missing-files` | 미등록 | ★ | Python·Rust 로 main 에서 재현, 수정안 검증 | 파티션 값에 `~` → optimize 실패. 이슈 먼저 올릴 것 |
| 7 | `iceberg-compat-v2-writer` | #3299 (C 안) | ★★★ | main `f67f1210` 에 프로토타입 적용, 새 테스트 6 개 + 전체 suite 통과. Delta Spark UniForm 변환 + pyiceberg 읽기까지 확인 | 2번의 확장. PR 4 개(PR 0: 재작성 파일 `ARROW:schema` 의 view 타입 수정, main 버그·이슈 먼저 → materializePartitionColumns → nested id → icebergCompatV2). 이슈에 방향 질문 먼저. `issue.md` 없음, `plan.md`·`plan-integration.md`·`pr.md`·`prototype.diff`·`integration/` |

## 해결됨 (done/)

| 폴더 | 이슈 | 해결 |
|---|---|---|
| `done/unity-catalog-null-expiration-time` | #3966 | PR 4884 머지(`3604f18a`, 2026-10-15), 이슈 close |
| `done/unity-catalog-unknown-enum-values` | #3997 | PR 4885 머지(`2b656390`, 2026-10-15), 이슈 close |

## 검토했지만 제외한 open issue (PR 없음)

- #4613 RESTORE+DV: main 에서 이미 수정됨(`d1039f9`). 테스트만 추가한 #4722 도 닫힘 → 이슈 close 요청만 가능.
- #4535 MERGE not-matched-by-source 파티션 프루닝: 메인테이너가 "full scan 필요" 로 #4596 거절.
- #4819 predicate 타입 통일: 설계 논의 중(#4676 스택과 연관).
- #3911 vacuum 이 파일을 안 지움: `dry_run=True` 기본값. 문서는 이미 설명함. 기본값 변경은 메인테이너 결정 필요.
- #3392 CDF vs vacuum, #4071 z-order 비증분, #3905 decimal stats 표현, #4021 `data_change`: 프로토콜/kernel/설계 이슈.
- #3906(DynamoDB), #4628(UC 토큰 갱신), #3203, #3852, #4401, #3991/#3951(Azure), #2425/#3551(Windows): 외부 환경 없이는 재현 불가.
- #4523/#4521: `ai` 라벨의 메모리 사용량 이슈, 측정 환경 필요.
- #2001: 이미 해결됨, close 요청만.
- #2146 경로 인코딩 테스트 확대: 범위가 정해지지 않은 umbrella 이슈. 같은 방향의 #4815 도 큰 리팩터링 제안이라 제외. 특수문자 파티션 값으로 write/read/optimize/vacuum/DML/checkpoint 를 돌려 본 결과 새로 찾은 버그는 6번 하나(vacuum 은 #4646·PR #4658, CDF 는 #4842·PR #4843 에서 이미 진행 중).
- #4864 parquet `metadata_size_hint` 미적용: #4870 으로 해결, 이슈 close.

## 공통 주의

- **upstream 에 링크 걸지 않기.** 이 브랜치는 `origin`(jx2lee/delta-rs) 에만 push 하고 `upstream` 에는 push 하지 않는다. 커밋 메시지에 `delta-io/delta-rs#123` 이나 upstream 이슈·PR URL 을 쓰면 upstream 이슈 타임라인에 "referenced this issue" 가 생기므로 쓰지 않는다. 커밋 메시지에서는 번호 없이 폴더 이름으로 가리킨다. 문서(.md) 본문 안의 링크는 타임라인에 남지 않으니 괜찮다.
- CONTRIBUTING.md: **AI 로만 만든 PR 은 닫힐 수 있음**(#4437 가 그 이유로 닫힘). 무엇을 직접 검증했는지 PR 본문에 적을 것.
- 이슈에 `take` 댓글 → 자동 할당. 커밋은 `git commit -s`(DCO). 제목은 소문자 conventional commits.
- 체크: `cargo fmt --all`, `cargo clippy -p <crate> --tests`, 해당 crate 테스트.
