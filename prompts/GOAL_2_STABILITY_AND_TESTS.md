# Goal 2 Prompt: Stability and Tests

Use this after Goal 1 is implemented.

```text
방금 만든 mzc Rust 프로젝트를 검토하고 개선해줘.

이번 목표는 코드 품질과 학습성을 높이는 것이다.

요구사항:
1. cargo test가 통과하는지 확인한다.
2. 압축/해제 roundtrip 테스트를 보강한다.
3. 빈 파일, 1바이트 파일, 반복이 많은 파일, 반복이 없는 파일, 바이너리 파일 테스트를 추가한다.
4. Literal Block이 u16 최대 길이를 넘는 경우에도 안전하게 나뉘도록 확인한다.
5. Run Block이 u16 최대 길이를 넘는 경우에도 여러 블록으로 나뉘도록 확인한다.
6. 잘못된 magic header, 잘못된 block type, payload 손상 케이스에 대한 에러 처리를 추가한다.
7. README.md의 사용 예시를 실제 명령어 기준으로 보강한다.
8. Rust 초보자 입장에서 중요한 부분에 주석을 추가한다.
9. 전체 구조를 유지하되 불필요하게 복잡한 코드는 단순화한다.

완료 조건:
- cargo build 성공
- cargo test 성공
- 모든 에러 케이스가 명확한 메시지로 처리됨
- 압축/해제 결과가 원본과 항상 동일함
```
