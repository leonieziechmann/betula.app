These reference snapshots were originally extracted with pdfplumber 0.11.9 from public BTU
regulations already downloaded by the scraper:

- Informatik B.Sc., ABl. 12/2024, Anlage 2, physical PDF page 7:
  https://opus4.kobv.de/opus4-btu/files/6707/12_Informatik_B.Sc.pdf
- Wirtschaftsinformatik B.Sc., ABl. 21/2024, Anlage 3, physical PDF page 8:
  https://opus4.kobv.de/opus4-btu/files/6749/21_Wirtschaftsinformatik.pdf

The expected semester assignments in validation_test.go were checked visually
against the rendered pages. They are deliberately independent of the AI response.
Informatik includes a vertically merged elective cell and horizontal 5–6 spans;
Wirtschaftsinformatik includes unnamed subtotal rows and repeated elective slots.

Optional integration tests compare the Go extractor to these snapshots.
Cell identity, labels, semester spans, credits, row indices, bold type and intake
must match exactly; border coordinates may differ by at most 0.25 PDF points due
to stroke averaging. `row_index` and `Bold` came to the reader after the snapshots
were taken. They were recorded from the Go reader (2026-09-23) and checked against
the PDFs: each row index is the row in the cell's id, and `Bold` is set exactly
where the PDF prints the value in Arial-BoldMT. Every other value is pdfplumber's.
Set RADIX_PDF_TEST_DIR to the absolute statutes directory. No Python runtime is used.
The default regression tests also generate real PDFs entirely in Go and do not
make API calls.
