# Goal 1 Prompt: Initial MZC1 Implementation

Use the prompt below in your vibe-coding tool.

```text
Rust 학습을 병행하면서 학습용 무손실 압축 CLI 도구를 만들고 싶다.

프로젝트 이름은 mzc이고, 의미는 Minimal Zip Concept이다.
목표는 기존 ZIP, Zstd, Brotli 같은 상용 압축 알고리즘을 이기는 것이 아니라, 압축 알고리즘의 원리를 이해하고 직접 설계한 포맷으로 압축/해제를 구현하는 것이다.

이번 1차 목표는 MZC1 포맷을 사용하는 RLE 기반 무손실 압축/해제 CLI 도구를 완성하는 것이다.

개발 언어:
- Rust

사용할 crate:
- clap = CLI 명령어 파싱
- sha2 = SHA-256 계산
- anyhow = 에러 처리

지원 명령어:
1. mzc compress <input_file> <output_file>
2. mzc decompress <input_file> <output_file>
3. mzc test <input_file>
4. mzc inspect <input_file>

MZC1 파일 포맷:
1. Magic Header       4 bytes   "MZC1"
2. Version            1 byte    0x01
3. Algorithm Type     1 byte    0x01 = RLE
4. Original Size      8 bytes   u64 little-endian
5. Payload Size       8 bytes   u64 little-endian
6. Original SHA-256   32 bytes
7. Payload            variable

Payload는 블록들의 연속으로 구성한다.

Block Type 0x00 = Literal Block:
- Type      1 byte
- Length    2 bytes, u16 little-endian
- Data      N bytes

Block Type 0x01 = Run Block:
- Type      1 byte
- Count     2 bytes, u16 little-endian
- Value     1 byte

압축 규칙:
- 같은 바이트가 4개 이상 연속되면 Run Block으로 저장한다.
- 그 외 바이트들은 Literal Block으로 묶어서 저장한다.
- Literal Block과 Run Block의 최대 길이는 u16 범위 안에서 처리한다.
- 큰 파일에서도 논리적으로 안전하게 동작하도록 구현한다.

검증 규칙:
- 압축 시 원본 SHA-256을 Header에 저장한다.
- 해제 후 복원된 데이터의 SHA-256과 Header의 SHA-256을 비교한다.
- 다르면 에러를 발생시킨다.
- test 명령어는 input_file을 임시로 압축 후 해제하고 원본과 복원본의 해시가 같은지 검증한다.

inspect 명령어:
- MZC1 파일을 읽고 아래 정보를 출력한다.
  - Magic Header
  - Version
  - Algorithm Type
  - Original Size
  - Payload Size
  - SHA-256
  - Estimated compression ratio

출력 예:
Original size: 102400 bytes
Compressed size: 38210 bytes
Ratio: 37.31%
Verified: OK

프로젝트 구조:
mzc/
├─ Cargo.toml
├─ README.md
├─ docs/
│  ├─ FORMAT_MZC1.md
│  ├─ ROADMAP.md
│  └─ TEST_PLAN.md
├─ samples/
│  ├─ repeated.txt
│  ├─ normal.txt
│  └─ binary_sample.bin
├─ src/
│  ├─ main.rs
│  ├─ cli.rs
│  ├─ format.rs
│  ├─ rle.rs
│  ├─ checksum.rs
│  ├─ inspect.rs
│  └─ error.rs
└─ tests/
   ├─ roundtrip_tests.rs
   └─ format_tests.rs

구현 요구사항:
1. 먼저 전체 설계를 간단히 설명해라.
2. 그다음 Rust 프로젝트를 생성하고 위 구조로 파일을 작성해라.
3. 각 모듈의 역할이 명확하게 분리되도록 구현해라.
4. 압축 후 해제한 데이터는 원본과 1바이트도 달라지면 안 된다.
5. 텍스트 파일뿐 아니라 바이너리 파일도 처리 가능해야 한다.
6. 압축 결과가 원본보다 커질 수 있는 경우도 정상 상황으로 처리하라.
7. 테스트 코드를 반드시 작성하라.
8. README.md에는 빌드 방법, 사용법, 포맷 설명, 예시 명령어를 포함하라.
9. docs/FORMAT_MZC1.md에는 파일 포맷을 표로 정리하라.
10. docs/ROADMAP.md에는 MZC2에서 사전 압축을 추가하는 계획을 작성하라.
11. docs/TEST_PLAN.md에는 테스트 케이스를 정리하라.

Rust 학습 관점에서 추가 요구:
- 코드에 과도한 추상화를 넣지 말고, 초보자가 따라가기 쉬운 구조로 작성해라.
- 중요한 Rust 문법에는 주석을 달아라.
- Result, enum, struct, Vec<u8>, slice, file I/O를 학습할 수 있도록 구현하라.
- 성능 최적화보다 정확성과 가독성을 우선하라.

완료 조건:
- cargo build 성공
- cargo test 성공
- repeated.txt 샘플은 압축률이 표시되어야 함
- normal.txt처럼 압축이 잘 안 되는 파일도 정상 처리되어야 함
- compress → decompress → SHA-256 검증이 성공해야 함
```
