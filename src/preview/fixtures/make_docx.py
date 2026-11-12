"""Generate the project's own deterministic two-page DOCX preview fixture."""
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED

parts = {
    "[Content_Types].xml": '''<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>''',
    "_rels/.rels": '''<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>''',
    "word/document.xml": '''<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
<w:p><w:r><w:rPr><w:b/><w:color w:val="2060A0"/><w:sz w:val="40"/></w:rPr><w:t>Formatted DOCX preview</w:t></w:r></w:p>
<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="8"/><w:bottom w:val="single" w:sz="8"/><w:insideH w:val="single" w:sz="8"/></w:tblBorders></w:tblPr>
<w:tblGrid><w:gridCol w:w="4500"/><w:gridCol w:w="4500"/></w:tblGrid>
<w:tr><w:tc><w:p><w:r><w:t>Field</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Layout</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Formatting retained</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:p><w:r><w:br w:type="page"/></w:r></w:p>
<w:p><w:r><w:rPr><w:b/><w:sz w:val="36"/></w:rPr><w:t>Second page preview</w:t></w:r></w:p>
<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1000" w:right="1000" w:bottom="1000" w:left="1000"/></w:sectPr>
</w:body></w:document>''',
}
with ZipFile(Path(__file__).with_name("two-pages.docx"), "w") as document:
    for name, xml in parts.items():
        info = ZipInfo(name, date_time=(2000, 1, 1, 0, 0, 0))
        info.compress_type = ZIP_DEFLATED
        document.writestr(info, '<?xml version="1.0" encoding="UTF-8"?>\n' + xml)
