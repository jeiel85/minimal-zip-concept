# Goal 3 Prompt: MZC2 Dictionary Compression Plan

Use this after MZC1 is stable.

```text
mzc 프로젝트의 다음 버전인 MZC2를 설계해줘.

목표:
RLE만으로는 압축률이 낮으므로, 텍스트/JSON/설교 원고/게임 데이터처럼 반복 패턴이 많은 파일에 더 잘 맞는 사전 기반 압축을 추가한다.

이번 단계에서는 구현 전에 설계를 먼저 완성한다.

요구사항:
1. MZC1과 MZC2의 차이를 정리한다.
2. MZC2 파일 포맷을 제안한다.
3. Dictionary Section 구조를 설계한다.
4. Payload Block에 Dictionary Token Block을 추가한다.
5. 기존 Literal Block과 Run Block을 유지할지 판단한다.
6. RLE only, Dictionary only, Hybrid 모드를 비교할 수 있는 구조를 제안한다.
7. 작은 파일에서 dictionary 저장 비용 때문에 오히려 커질 수 있는 문제를 설명하고 대응 방안을 제시한다.
8. UTF-8 텍스트뿐 아니라 바이너리 파일도 고려한다.
9. 압축 후 해제 결과는 반드시 원본과 1바이트도 달라지지 않아야 한다.
10. MZC2 구현을 위한 단계별 개발 계획을 작성한다.

문서 산출물:
- docs/FORMAT_MZC2_DRAFT.md
- docs/MZC2_ALGORITHM_DESIGN.md
- docs/MZC2_TEST_PLAN.md
- docs/MZC1_TO_MZC2_MIGRATION.md

주의:
- 바로 구현하지 말고 설계를 먼저 작성한다.
- 성능 최적화보다 포맷 안정성, 검증 가능성, 학습성을 우선한다.
```
