# delta-rs 기여 후보 (2026-10-13, main `b597cf23`)

조건: GitHub 에 등록된 **open issue** 이고, 링크된 open/merged PR 이 없으며, 소스에서 원인을 확인하고 재현한 것.
각 폴더에 `issue.md`(이슈 요약·재현 결과), `pr.md`(브랜치·제목·본문 초안), `guide.md`(수정 방법, Rust 초심자용) 가 있습니다.
`_repro/` 에는 재현에 실제로 쓴 테스트 코드가 있습니다(저장소에서는 되돌렸음).

| # | 폴더 | 이슈 | 난이도 | 재현 방식 | 비고 |
|---|---|---|---|---|---|
| 1 | `parquet-metadata-size-hint-not-applied` | #4864 | ★ | DataFusion 55.1 소스 대조 | 10줄 fix, 가장 먼저 추천 |
| 2 | `unity-catalog-null-expiration-time` | #3966 | ★ | serde 단위 테스트 실패 확인 | 필드 하나 Option 화 |
| 3 | `unity-catalog-unknown-enum-values` | #3997 | ★ | serde 단위 테스트 실패 확인 | enum 에 `#[serde(other)]` |
| 4 | `uniform-table-properties-rejected` | #3299 | ★★ | in-memory create 로 에러 재현 | enum variant + writer feature |
| 5 | `zorder-dotted-column-names` | #4363 | ★★ | in-memory z-order 로 에러 재현 | kernel `ColumnName` 파서 사용. 이전 `take` 있음 |
| 6 | `compaction-ordered-buffer-delays-commits` | #4749 | ★★ | 코드·git 이력 대조 | 1줄 fix 가능하나 먼저 이슈에 방향 질문 |
| 7 | `nested-schema-override-widened-in-parquet-read` | #4678 | ★★★★ | `plan.rs` 단위 테스트 실패 확인 | 닫힌 PR 2개의 리뷰 코멘트 반영 필요 |

## 검토했지만 제외한 open issue (PR 없음)

- #4613 RESTORE+DV: main 에서 이미 수정됨(`d1039f9`). 테스트만 추가한 #4722 도 닫힘 → 이슈 close 요청만 가능.
- #4535 MERGE not-matched-by-source 파티션 프루닝: 메인테이너가 "full scan 필요" 로 #4596 거절.
- #4819 predicate 타입 통일: 설계 논의 중(#4676 스택과 연관).
- #3911 vacuum 이 파일을 안 지움: `dry_run=True` 기본값. 문서는 이미 설명함. 기본값 변경은 메인테이너 결정 필요.
- #3392 CDF vs vacuum, #4071 z-order 비증분, #3905 decimal stats 표현, #4021 `data_change`: 프로토콜/kernel/설계 이슈.
- #3906(DynamoDB), #4628(UC 토큰 갱신), #3203, #3852, #4401, #3991/#3951(Azure), #2425/#3551(Windows): 외부 환경 없이는 재현 불가.
- #4523/#4521: `ai` 라벨의 메모리 사용량 이슈, 측정 환경 필요.
- #2001: 이미 해결됨, close 요청만.

## 공통 주의

- CONTRIBUTING.md: **AI 로만 만든 PR 은 닫힐 수 있음**(#4437 가 그 이유로 닫힘). 무엇을 직접 검증했는지 PR 본문에 적을 것.
- 이슈에 `take` 댓글 → 자동 할당. 커밋은 `git commit -s`(DCO). 제목은 소문자 conventional commits.
- 체크: `cargo fmt --all`, `cargo clippy -p <crate> --tests`, 해당 crate 테스트.
