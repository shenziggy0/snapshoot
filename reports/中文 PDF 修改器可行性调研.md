# 中文 PDF 修改器可以从改一个词做起

**能做。建议第一版做本地桌面工具，让用户点选已有的中文姓名、日期或错字，在原处替换，预览后另存。** 这能解决手里只有 PDF、却要改一个词的问题。只要新词能放进原来的位置，就可以把整段换行和跨页排版留到以后。本次按普通中文文件评估，目标操作系统和预算待定。

## 填表、改正文和改扫描件，需要三种不同的能力

| 用户想做什么 | 开发时要解决什么 |
|---|---|
| 加批注、填表、拆页或合并 | 最容易起步。批注可以叠加在页面上，有输入框的表单可以直接填，页面也能单独整理。在旧字上盖白块会留下可搜索的原文。[批注工具](https://support.apple.com/zh-cn/guide/preview/prvw11580/mac) · [表单与页面示例](https://pdf-lib.js.org/) |
| 替换一行里已有的几个中文字 | **最适合首版。** 先找准并删掉选中的旧字，再按原位置写入新词。表格里有些字各自带坐标，程序还要检查字体有没有新字、新词会不会压到隔壁的格子。[文字位置说明](https://pypdf.readthedocs.io/en/stable/user/extract-text.html#why-text-extraction-is-hard) |
| 改扫描件，或把一段扩写到下一页 | 扫描页上的字是图片的一部分。文字识别（OCR）能增加搜索层，要让打印出来的字也改变，还需擦掉旧笔画、补背景。长段改写则要决定哪里换行、后面的表格往哪移。即使 Acrobat 的跨页重排也受内容和语言限制。[OCR 文字层](https://ocrmypdf.readthedocs.io/en/latest/cookbook.html#remove-the-ocr-text-layer-from-my-pdf) · [Adobe 自动调整布局](https://experienceleague.adobe.com/zh-hans/docs/document-cloud-learn/acrobat-learning/get-started/edit/auto-adjust-layout) |

## 「周二」改成了「周五」，整页提取顺序还要处理

我们做了一份两页中文样本。第 1 页是会议表格，文件里的字体只收录了部分字，恰好缺「五」。用 PyMuPDF 删除「周二」，从本机同款完整字体补入所需字形，再在原处写入「周五」。另存重开后，**4 个工具都提取到一次「周五」、零次「周二」**。3 个渲染引擎分别以 144 dpi 对比改前和改后，改字区域外均有 **0 个像素变化**，隔壁的「下午三点」和表格线都保住了。（[改前 PDF](<../research_notes/中文 PDF 修改器可行性调研/改前.pdf>) · [改后 PDF](<../research_notes/中文 PDF 修改器可行性调研/改后.pdf>)）

画面正确，整页文字仍可能乱序。MuPDF 和 pypdf 默认提取时都把「周五」放到了页末。第 2 页则是同一张表的纯图片对照，肉眼能看到「周二」，4 个工具都提取到零个文字字符。本次未测试 OCR 或扫描件改字。（[实测记录](<../research_notes/中文 PDF 修改器可行性调研/中文样本实测.md>)）

## 原型用 PyMuPDF，按发行方式选引擎

**pdf-lib** 能拆并页面、填写预设输入框，修改既有文字的接口限于表单字段。本地改字原型可以沿用这次跑通的 **PyMuPDF**，官方提供 AGPLv3 和商业授权。若已确定闭源分发，建议在正式开发前评估许可，免得做完再换引擎。（[pdf-lib 能力边界](https://github.com/Hopding/pdf-lib#limitations) · [PyMuPDF 授权](https://pymupdf.io/licensing)）

准备闭源发行时，也可评估 **PDFium**。它提供改写文字对象的接口，主许可采用 BSD 三条款，文字选取、中文补字仍要自己实现。希望直接集成段落编辑界面，可试 **Apryse WebViewer**，其 PDF Editing 是单独授权的附加组件。（[PDFium 接口](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_edit.h) · [许可原文](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/LICENSE) · [Apryse 编辑组件](https://docs.apryse.com/web/guides/edit/text-edit)）

## 第一版怎样算做成

拿实际要改的中文文件验收。用户选词、输入、预览、另存，再用另一款阅读器打开。新词应能显示、搜索和复制，原位置应搜不到旧词，旁边的字和线条应完整。缺原字体时预览替代字体，位置放不下时提示缩短。若产品也承诺整页复制，就把「会议日期、周五、下午三点」能按原顺序读出列为发布条件。
