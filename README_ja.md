# zipbit11
ZIPエントリの汎用ビットフラグにおけるビット11（UTF-8フラグ）を変更するCLIツール

![zipbit11](https://github.com/user-attachments/assets/e1a17bcb-de7d-4fa8-a61e-2f21b71f557e)

[![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/yuk7/zipbit11/ci.yml?style=flat-square)](https://github.com/yuk7/zipbit11/actions/workflows/ci.yml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=flat-square)](http://makeapullrequest.com)
![License](https://img.shields.io/github/license/yuk7/zipbit11.svg?style=flat-square)

[English](README.md)

### [⬇ダウンロード](https://github.com/yuk7/zipbit11/releases/latest)

## このツールを使う理由
ZIPフォーマットには、ファイル名がUTF-8でエンコードされていることを示す「汎用ビットフラグのビット11（UTF-8フラグ）」が存在します。しかし、実際にはUTF-8で作成されたZIPファイルでも、このフラグが設定されていないことがあります。

ZIPを展開するツールやOSによっては、UTF-8フラグが立っていない場合にファイル名を別のエンコーディングとして解釈することがあり、ファイル名が文字化けする原因になることがあります。

このツールを使えば、ZIPファイルを直接編集してUTF-8フラグを付与（または除去）できます。ファイルを渡す前や受け取った後にひと手間加えるだけで、文字化けを解消できることがあります。


## 注意
このツールは直接zipファイルを編集します。
大切なファイルは必ずバックアップを取ってから使用してください。


## 使い方
```bash
zipbit11 <コマンド> <ファイル.zip> [エントリ]
zipbit11 help
```

### コマンド
- `status`: エントリ数とビット11の全体的なサマリーを表示する
- `detail`: 全エントリ（または指定した `[エントリ]`）のサマリーとビット11の状態を表示する
- `set`: 全エントリ（または指定した `[エントリ]`）のビット11をセットする
- `clear`: 全エントリ（または指定した `[エントリ]`）のビット11をクリアする
- `toggle`: 全エントリ（または指定した `[エントリ]`）のビット11をトグルする
- `help`: ヘルプを表示する

### エントリセレクタ
- `detail` の行番号をカンマ区切りの値や両端を含む範囲で指定できます（例：`1,3,5-8`）

## 使用例
### zip内のコンテンツがUTF-8であることを明示
```bash
zipbit11 set archive.zip
```

### zip内のコンテンツがUTF-8であるという明示を解除
```bash
zipbit11 clear archive.zip
```
### zip内のコンテンツの状態を確認
```bash
zipbit11 detail archive.zip

# 出力
File: archive.zip
Entries: 4
bit11: △ partial (2/4)

 No.   bit11   Filename
 ------------------------------------------------------------
 1     ✗ clear  English/
 2     ✗ clear  English/cat.txt
 3     ✓ set    日本語/
 4     ✓ set    日本語/猫.txt
```

## ライセンス
[MIT](LICENSE)
