# MZC2 File Format Specification (Draft)

MZC2는 RLE(Run-Length Encoding) 기반의 MZC1 포맷을 대폭 확장하여, 중복 단어와 반복 패턴이 대량으로 등장하는 텍스트 파일(설교 원고, JSON 데이터, 게임 설정문 등) 및 고정 바이너리 반복 프레임의 압축률을 극대화하도록 설계된 **사전 기반(Dictionary-based) 하이브리드 무손실 압축 포맷**입니다.

이 규격은 포맷의 안정성, 학습성, 안전한 예외 제어 능력을 최우선 가치로 두고 설계되었습니다.

---

## 1. MZC2 전체 파일 레이아웃 (File Layout)

MZC2 압축 파일은 크게 세 영역으로 구분됩니다:

```text
+-----------------------+
|  MZC2 파일 헤더       |  56바이트 고정 영역
+-----------------------+
|  사전 섹션            |  가변 크기 (헤더의 Dictionary Size 만큼 차지)
| (Dictionary Section)  |
+-----------------------+
|  압축 페이로드 블록   |  가변 크기 (헤더의 Payload Size 만큼 차지)
|  (Payload Blocks)     |  (Literal / Run / Token 블록 체인)
+-----------------------+
```

---

## 2. MZC2 파일 헤더 명세 (Header Layout - Fixed 56 Bytes)

MZC2 헤더는 MZC1(54바이트)에서 사전의 경계를 명확하게 식별하고 개별 분석하기 위해 **Dictionary Size (2바이트)** 필드를 추가하여 총 **56바이트**의 고정 크기를 지닙니다.

| 필드명 (Field) | 크기 (Size) | 타입 (Type) | 고정값 / 설명 (Description) |
| :--- | :---: | :---: | :--- |
| **Magic Header** | 4 bytes | ASCII | 반드시 `"MZC2"` 문자열이어야 함 |
| **Version** | 1 byte | u8 | `0x02` (MZC2 버전 표시) |
| **Algorithm Type** | 1 byte | u8 | 작동 방식 결정:<br>- `0x01`: RLE Only (사전 없음, MZC1과 하위 호환 모드)<br>- `0x02`: Dictionary Only (토큰 및 리터럴 블록만 사용)<br>- `0x03`: Hybrid Mode (RLE, Literal, Token 블록을 모두 교차 혼용) |
| **Original Size** | 8 bytes | u64 | 압축 전 원본 데이터 전체의 바이트 크기 (Little-Endian) |
| **Payload Size** | 8 bytes | u64 | 압축 페이로드 블록 영역만의 순수 크기 (Little-Endian) |
| **Dictionary Size** | 2 bytes | u16 | 사전 섹션(Dictionary Section)의 전체 바이트 크기 (Little-Endian) |
| **Original SHA-256** | 32 bytes | bytes | 원본 데이터 전체에 대한 SHA-256 무손실 복원 검증용 해시 |

---

## 3. 사전 섹션 구조 (Dictionary Section Layout)

사전 섹션은 가변 크기 바이트 시퀀스를 항목으로 갖는 **바이트 지향적 사전(Byte-oriented Dictionary)**입니다. UTF-8 문자열 뿐만 아니라 일반 바이너리 데이터 청크(예: 게임 그래픽 프레임 패턴, 이미지 자산 시퀀스 등)도 담을 수 있습니다.

사전의 최대 단어 개수는 `u16::MAX` (65,535개)로 제약되며, 각 사전 엔트리(Entry)의 최대 길이 또한 `u8::MAX` (255바이트)로 제한하여 사전 데이터 파싱 효율을 높이고 구조를 단순화합니다.

### 3.1 사전 섹션 이진 구성도

```text
+-----------------------+---------------------------------------+
|  Entry Count (2 bytes) |  사전에 등록된 단어/패턴의 총 개수 ($M$) | u16 (Little-Endian)
+-----------------------+---------------------------------------+
|  Entry #0 Length      |  첫 번째 단어의 바이트 길이 ($L_0$)     | u8 (1 ~ 255)
+-----------------------+---------------------------------------+
|  Entry #0 Data        |  첫 번째 단어의 원본 바이트 배열       | $L_0$ bytes
+-----------------------+---------------------------------------+
|  Entry #1 Length      |  두 번째 단어의 바이트 길이 ($L_1$)     | u8 (1 ~ 255)
+-----------------------+---------------------------------------+
|  Entry #1 Data        |  두 번째 단어의 원본 바이트 배열       | $L_1$ bytes
+-----------------------+---------------------------------------+
|  ...                  |  ...                                  | ...
+-----------------------+---------------------------------------+
|  Entry #M-1 Length    |  마지막 단어의 바이트 길이 ($L_{M-1}$) | u8 (1 ~ 255)
+-----------------------+---------------------------------------+
|  Entry #M-1 Data      |  마지막 단어의 원본 바이트 배열       | $L_{M-1}$ bytes
+-----------------------+---------------------------------------+
```

* **Index 할당 방식:** 사전에 수록된 순서대로 0번 인덱스부터 $M-1$번 인덱스까지의 `u16` 정수형 주소가 부여됩니다.
* **학습성 요소:** 포인터 연산 없이 사전 데이터를 순차 분석(Linear Parsing)하는 방식으로 쉽게 파서 코드를 설계할 수 있습니다.

---

## 4. 압축 페이로드 블록 명세 (Payload Blocks Specification)

페이로드 영역은 가변 크기의 이진 블록 체인입니다. MZC2 포맷은 MZC1의 기존 블록 외에 사전 토큰 매칭을 지시하는 **Token Block (0x02)**을 추가로 정의합니다.

### 4.1 Block Type 0x00: Literal Block (리터럴 블록)
사전이나 RLE 연속 처리에 걸려들지 못한 일반 바이트 영역을 보존합니다.
* **Type** (1 byte): `0x00`
* **Length** (2 bytes, `u16` Little-Endian): 데이터 바이트 길이 $N$ (최대 65,535)
* **Data** ($N$ bytes): 원본 데이터 스트림

### 4.2 Block Type 0x01: Run Block (런 블록)
특정 동일 바이트가 연속으로 대량 반복될 때 사용합니다. (하이브리드 모드 시 사용)
* **Type** (1 byte): `0x01`
* **Count** (2 bytes, `u16` Little-Endian): 반복 횟수 (최대 65,535)
* **Value** (1 byte): 반복될 단일 바이트 값

### 4.3 Block Type 0x02: Token Block (토큰 블록)
사전에 정의되어 수록되어 있는 특정 패턴 바이트 배열을 지목하여 치환합니다.
* **Type** (1 byte): `0x02`
* **Token Index** (2 bytes, `u16` Little-Endian): 사전의 인덱스 주소 (0 ~ 65,535)

---

## 5. 포맷 안정성 및 예외 처리 검증 규칙 (Safety & Invalid Cases)

디코더는 MZC1의 예외 처리에 더해 다음과 같은 MZC2 특유의 포맷 훼손 케이스를 엄격하게 차단하고 명확한 오류 코드를 발생시켜야 합니다.

1. **사전 경계 이탈 (Dictionary Out-of-Bounds)**:
   * 사전 섹션을 디코딩하는 중, 헤더에 기재된 `Dictionary Size` 바이트 한계치를 지나치거나 부족하게 데이터가 잘린 경우 (`MzcError::CorruptDictionary`)
2. **사전 엔트리 오버플로우 (Dictionary Entry Overflow)**:
   * 사전에 선언된 `Entry Count` 대비 실제 읽어낸 엔트리 수가 다르거나 데이터가 잘린 경우
3. **잘못된 토큰 인덱스 (Invalid Token Index)**:
   * 디코딩 중 `Token Block(0x02)`을 만났으나, `Token Index`가 사전에 정의된 총 단어 개수 $M$ 이상일 경우 (`MzcError::InvalidTokenIndex`)
4. **알고리즘 타입 부정합 (Algorithm Mismatch)**:
   * 헤더의 `Algorithm Type`이 `0x01`(RLE)인데 본문에 `0x02`(Token) 블록이 출현하거나, `Algorithm Type`이 `0x02`(Dict)인데 `0x01`(Run) 블록이 출현할 때 포맷 부정합으로 즉시 차단
