use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use tauri::State;

use super::{finance, AppError, Database};

const PAGE_WIDTH: f32 = 595.28;
const PAGE_HEIGHT: f32 = 841.89;
const MARGIN: f32 = 48.0;
const FOOTER_Y: f32 = 37.0;
const CONTENT_RIGHT: f32 = PAGE_WIDTH - MARGIN;
const COLOR_INK: (f32, f32, f32) = (0.07, 0.12, 0.09);
const COLOR_MUTED: (f32, f32, f32) = (0.37, 0.43, 0.39);
const COLOR_LINE: (f32, f32, f32) = (0.84, 0.88, 0.85);
const COLOR_SOFT: (f32, f32, f32) = (0.95, 0.98, 0.96);
const COLOR_GREEN: (f32, f32, f32) = (0.08, 0.57, 0.31);
const COLOR_WHITE: (f32, f32, f32) = (1.0, 1.0, 1.0);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PdfExportResult {
    destination_path: String,
    file_name: String,
    page_count: usize,
    bytes_written: usize,
}

#[derive(Debug, Clone)]
struct Contact {
    name: String,
    email: Option<String>,
    address: Option<String>,
}

#[derive(Debug, Clone)]
struct InvoiceHeader {
    number: String,
    status: String,
    issue_date: String,
    due_date: String,
    currency: String,
    subtotal_minor: i64,
    discount_total_minor: i64,
    tax_total_minor: i64,
    total_minor: i64,
    notes: Option<String>,
    payment_instructions: Option<String>,
    seller: Contact,
    bill_to: Contact,
    project_name: Option<String>,
    milestone_kind: String,
    milestone_percent_basis_points: Option<i64>,
    milestone_label: Option<String>,
}

#[derive(Debug, Clone)]
struct InvoiceItem {
    description: String,
    quantity_millis: i64,
    unit_price_minor: i64,
    line_total_minor: i64,
}

#[derive(Debug, Clone)]
struct InvoiceAdjustment {
    kind: String,
    label: String,
    calculation_type: String,
    rate_basis_points: Option<i64>,
    amount_minor: i64,
}

#[derive(Debug, Clone)]
struct InvoicePayment {
    amount_minor: i64,
    paid_at: String,
    payment_method: Option<String>,
    reference: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Clone)]
struct InvoiceDocument {
    header: InvoiceHeader,
    items: Vec<InvoiceItem>,
    adjustments: Vec<InvoiceAdjustment>,
    payments: Vec<InvoicePayment>,
}

#[tauri::command]
pub(crate) fn export_invoice_pdf(
    database: State<'_, Database>,
    invoice_id: String,
    destination_path: String,
) -> Result<PdfExportResult, AppError> {
    let invoice_id = invoice_id.trim();
    if invoice_id.is_empty() {
        return Err(AppError::InvalidInput(
            "Invoice ID is required for PDF export.".into(),
        ));
    }
    let path = validate_destination(&destination_path)?;
    let document = {
        let connection = database.lock()?;
        finance::ensure_finance_schema_compatibility(&connection)?;
        load_invoice_document(&connection, invoice_id)?
    };

    let pages = render_invoice(&document);
    let pdf = build_pdf(&pages);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)?;
    file.write_all(&pdf)?;
    file.flush()?;

    Ok(PdfExportResult {
        destination_path: path.to_string_lossy().into_owned(),
        file_name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}.pdf", document.header.number)),
        page_count: pages.len(),
        bytes_written: pdf.len(),
    })
}

fn validate_destination(destination: &str) -> Result<PathBuf, AppError> {
    let trimmed = destination.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput(
            "Choose a destination for the invoice PDF.".into(),
        ));
    }
    let path = PathBuf::from(trimmed);
    let is_pdf = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"));
    if !is_pdf {
        return Err(AppError::InvalidInput(
            "Invoice exports must use a .pdf file extension.".into(),
        ));
    }
    if path.file_name().is_none() {
        return Err(AppError::InvalidInput(
            "The PDF destination must include a file name.".into(),
        ));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            AppError::InvalidInput(
                "The PDF destination must include an existing parent folder.".into(),
            )
        })?;
    let metadata = fs::metadata(parent).map_err(|_| {
        AppError::InvalidInput(format!(
            "The PDF destination folder does not exist: {}",
            parent.display()
        ))
    })?;
    if !metadata.is_dir() {
        return Err(AppError::InvalidInput(
            "The PDF destination parent must be a folder.".into(),
        ));
    }
    if path.exists() && !path.is_file() {
        return Err(AppError::InvalidInput(
            "The PDF destination points to a folder.".into(),
        ));
    }
    Ok(path)
}

fn load_invoice_document(
    connection: &Connection,
    invoice_id: &str,
) -> Result<InvoiceDocument, AppError> {
    let header = connection
        .query_row(
            "SELECT
                i.invoice_number,
                i.status,
                i.issue_date,
                i.due_date,
                i.currency,
                i.subtotal_minor,
                i.discount_total_minor,
                i.tax_total_minor,
                i.total_minor,
                i.notes,
                i.payment_instructions,
                i.bill_to_name,
                i.bill_to_email,
                i.bill_to_address,
                i.seller_name,
                i.seller_email,
                i.seller_address,
                i.milestone_kind,
                i.milestone_percent_basis_points,
                i.milestone_label,
                COALESCE(NULLIF(TRIM(i.project_name), ''), p.name)
             FROM invoices i
             LEFT JOIN projects p ON p.id = i.project_id
             WHERE i.id = ?1 AND i.deleted_at IS NULL",
            params![invoice_id],
            invoice_header_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Invoice not found.".into()))?;
    if header.seller.name.trim().is_empty() {
        return Err(AppError::InvalidInput(
            "This invoice needs a seller display or business name before export. Open Settings → Invoice profile, add one, then create a new invoice."
                .into(),
        ));
    }

    let items = {
        let mut statement = connection.prepare(
            "SELECT description, quantity_millis, unit_price_minor, line_total_minor
             FROM invoice_items
             WHERE invoice_id = ?1 AND deleted_at IS NULL
             ORDER BY sort_order ASC, created_at ASC",
        )?;
        let rows = statement.query_map(params![invoice_id], |row| {
            Ok(InvoiceItem {
                description: row.get(0)?,
                quantity_millis: row.get(1)?,
                unit_price_minor: row.get(2)?,
                line_total_minor: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let adjustments = {
        let mut statement = connection.prepare(
            "SELECT kind, label, calculation_type, rate_basis_points, amount_minor
             FROM invoice_adjustments
             WHERE invoice_id = ?1 AND deleted_at IS NULL
             ORDER BY sort_order ASC, created_at ASC",
        )?;
        let rows = statement.query_map(params![invoice_id], |row| {
            Ok(InvoiceAdjustment {
                kind: row.get(0)?,
                label: row.get(1)?,
                calculation_type: row.get(2)?,
                rate_basis_points: row.get(3)?,
                amount_minor: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let payments = {
        let mut statement = connection.prepare(
            "SELECT amount_minor, paid_at, payment_method, reference, notes
             FROM invoice_payments
             WHERE invoice_id = ?1 AND deleted_at IS NULL
             ORDER BY paid_at ASC, created_at ASC",
        )?;
        let rows = statement.query_map(params![invoice_id], |row| {
            Ok(InvoicePayment {
                amount_minor: row.get(0)?,
                paid_at: row.get(1)?,
                payment_method: row.get(2)?,
                reference: row.get(3)?,
                notes: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    Ok(InvoiceDocument {
        header,
        items,
        adjustments,
        payments,
    })
}

fn invoice_header_from_row(row: &Row<'_>) -> rusqlite::Result<InvoiceHeader> {
    let snapshot_bill_name: String = row.get(11)?;
    let snapshot_bill_email: Option<String> = row.get(12)?;
    let snapshot_bill_address: Option<String> = row.get(13)?;
    let snapshot_seller_name: String = row.get(14)?;
    let snapshot_seller_email: Option<String> = row.get(15)?;
    let snapshot_seller_address: Option<String> = row.get(16)?;

    Ok(InvoiceHeader {
        number: row.get(0)?,
        status: row.get(1)?,
        issue_date: row.get(2)?,
        due_date: row.get(3)?,
        currency: row.get(4)?,
        subtotal_minor: row.get(5)?,
        discount_total_minor: row.get(6)?,
        tax_total_minor: row.get(7)?,
        total_minor: row.get(8)?,
        notes: row.get(9)?,
        payment_instructions: nonempty(row.get(10)?),
        seller: Contact {
            name: snapshot_seller_name.trim().to_owned(),
            email: nonempty(snapshot_seller_email),
            address: nonempty(snapshot_seller_address),
        },
        bill_to: Contact {
            name: snapshot_bill_name.trim().to_owned(),
            email: nonempty(snapshot_bill_email),
            address: nonempty(snapshot_bill_address),
        },
        project_name: nonempty(row.get(20)?),
        milestone_kind: row.get(17)?,
        milestone_percent_basis_points: row.get(18)?,
        milestone_label: nonempty(row.get(19)?),
    })
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

#[derive(Default)]
struct Canvas {
    content: String,
}

impl Canvas {
    fn text(&mut self, x: f32, y: f32, size: f32, bold: bool, color: (f32, f32, f32), value: &str) {
        let font = if bold { "F2" } else { "F1" };
        self.content.push_str(&format!(
            "BT {:.3} {:.3} {:.3} rg /{} {:.2} Tf 1 0 0 1 {:.2} {:.2} Tm {} Tj ET\n",
            color.0,
            color.1,
            color.2,
            font,
            size,
            x,
            y,
            pdf_literal(value)
        ));
    }

    fn text_right(
        &mut self,
        right: f32,
        y: f32,
        size: f32,
        bold: bool,
        color: (f32, f32, f32),
        value: &str,
    ) {
        let width = text_width(value, size, bold);
        self.text((right - width).max(MARGIN), y, size, bold, color, value);
    }

    fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: (f32, f32, f32)) {
        self.content.push_str(&format!(
            "q {:.3} {:.3} {:.3} RG {:.2} w {:.2} {:.2} m {:.2} {:.2} l S Q\n",
            color.0, color.1, color.2, width, x1, y1, x2, y2
        ));
    }

    fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: (f32, f32, f32)) {
        self.content.push_str(&format!(
            "q {:.3} {:.3} {:.3} rg {:.2} {:.2} {:.2} {:.2} re f Q\n",
            color.0, color.1, color.2, x, y, width, height
        ));
    }
}

struct InvoiceRenderer {
    pages: Vec<Canvas>,
    current: Canvas,
    y: f32,
}

impl InvoiceRenderer {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            current: Canvas::default(),
            y: PAGE_HEIGHT - MARGIN,
        }
    }

    fn page_break(&mut self, invoice: &InvoiceDocument) {
        let finished = std::mem::take(&mut self.current);
        if !finished.content.is_empty() {
            self.pages.push(finished);
        }
        self.current.text(
            MARGIN,
            PAGE_HEIGHT - MARGIN + 1.0,
            12.0,
            true,
            COLOR_INK,
            &invoice.header.seller.name,
        );
        self.current.text_right(
            CONTENT_RIGHT,
            PAGE_HEIGHT - MARGIN + 1.0,
            9.0,
            true,
            COLOR_MUTED,
            &format!("{}  ·  CONTINUED", invoice.header.number),
        );
        self.current.line(
            MARGIN,
            PAGE_HEIGHT - MARGIN - 15.0,
            CONTENT_RIGHT,
            PAGE_HEIGHT - MARGIN - 15.0,
            1.0,
            COLOR_GREEN,
        );
        self.y = PAGE_HEIGHT - MARGIN - 40.0;
    }

    fn ensure_space(&mut self, required: f32, invoice: &InvoiceDocument) -> bool {
        if self.y - required < FOOTER_Y + 25.0 {
            self.page_break(invoice);
            return true;
        }
        false
    }

    fn finish(mut self, invoice: &InvoiceDocument) -> Vec<String> {
        if !self.current.content.is_empty() {
            self.pages.push(self.current);
        }
        let page_count = self.pages.len();
        for (index, page) in self.pages.iter_mut().enumerate() {
            page.line(
                MARGIN,
                FOOTER_Y + 12.0,
                CONTENT_RIGHT,
                FOOTER_Y + 12.0,
                0.6,
                COLOR_LINE,
            );
            page.text(
                MARGIN,
                FOOTER_Y,
                7.5,
                false,
                COLOR_MUTED,
                &format!(
                    "{}  ·  {}",
                    invoice.header.seller.name, invoice.header.number
                ),
            );
            page.text_right(
                CONTENT_RIGHT,
                FOOTER_Y,
                7.5,
                false,
                COLOR_MUTED,
                &format!("Page {} of {}", index + 1, page_count),
            );
        }
        self.pages.into_iter().map(|page| page.content).collect()
    }
}

fn render_invoice(invoice: &InvoiceDocument) -> Vec<String> {
    let mut renderer = InvoiceRenderer::new();
    render_document_header(&mut renderer, invoice);
    render_contacts(&mut renderer, invoice);
    render_items(&mut renderer, invoice);
    render_totals(&mut renderer, invoice);
    render_payments(&mut renderer, invoice);
    if let Some(notes) = invoice.header.notes.as_deref() {
        render_text_section(&mut renderer, invoice, "NOTES", notes);
    }
    if let Some(instructions) = invoice.header.payment_instructions.as_deref() {
        render_text_section(&mut renderer, invoice, "PAYMENT INSTRUCTIONS", instructions);
    }
    renderer.finish(invoice)
}

fn render_document_header(renderer: &mut InvoiceRenderer, invoice: &InvoiceDocument) {
    let header = &invoice.header;
    renderer
        .current
        .fill_rect(MARGIN, 785.0, 5.0, 22.0, COLOR_GREEN);
    renderer.current.text(
        MARGIN + 14.0,
        793.0,
        15.0,
        true,
        COLOR_INK,
        &header.seller.name,
    );
    if let Some(email) = header.seller.email.as_deref() {
        renderer
            .current
            .text(MARGIN + 14.0, 778.0, 8.5, false, COLOR_MUTED, email);
    }
    renderer
        .current
        .text_right(CONTENT_RIGHT, 790.0, 25.0, true, COLOR_GREEN, "INVOICE");
    renderer
        .current
        .text_right(CONTENT_RIGHT, 770.0, 10.0, true, COLOR_INK, &header.number);
    renderer.current.text_right(
        CONTENT_RIGHT,
        754.0,
        8.0,
        true,
        COLOR_MUTED,
        &display_status(&header.status).to_uppercase(),
    );
    renderer
        .current
        .line(MARGIN, 730.0, CONTENT_RIGHT, 730.0, 1.2, COLOR_GREEN);

    render_meta_value(
        &mut renderer.current,
        MARGIN,
        703.0,
        "ISSUED",
        &display_date(&header.issue_date),
    );
    render_meta_value(
        &mut renderer.current,
        162.0,
        703.0,
        "DUE",
        &display_date(&header.due_date),
    );
    render_meta_value(
        &mut renderer.current,
        276.0,
        703.0,
        "PROJECT",
        header.project_name.as_deref().unwrap_or("General services"),
    );
    render_meta_value(
        &mut renderer.current,
        458.0,
        703.0,
        "MILESTONE",
        &milestone_label(header),
    );
    renderer.y = 665.0;
}

fn render_meta_value(canvas: &mut Canvas, x: f32, y: f32, label: &str, value: &str) {
    canvas.text(x, y, 7.5, true, COLOR_MUTED, label);
    let max_width = if x >= 458.0 { 89.0 } else { 160.0 };
    let visible = truncate_to_width(value, max_width, 9.5, true);
    canvas.text(x, y - 16.0, 9.5, true, COLOR_INK, &visible);
}

fn render_contacts(renderer: &mut InvoiceRenderer, invoice: &InvoiceDocument) {
    let seller_lines = contact_lines(&invoice.header.seller, 225.0);
    let bill_lines = contact_lines(&invoice.header.bill_to, 225.0);
    renderer
        .current
        .text(MARGIN, renderer.y, 7.5, true, COLOR_GREEN, "FROM");
    renderer
        .current
        .text(312.0, renderer.y, 7.5, true, COLOR_GREEN, "BILL TO");
    renderer.y -= 18.0;

    let count = seller_lines.len().max(bill_lines.len());
    for index in 0..count {
        if renderer.ensure_space(15.0, invoice) {
            renderer.current.text(
                MARGIN,
                renderer.y,
                7.5,
                true,
                COLOR_GREEN,
                "FROM · CONTINUED",
            );
            renderer.current.text(
                312.0,
                renderer.y,
                7.5,
                true,
                COLOR_GREEN,
                "BILL TO · CONTINUED",
            );
            renderer.y -= 18.0;
        }
        if let Some((line, bold)) = seller_lines.get(index) {
            renderer.current.text(
                MARGIN,
                renderer.y,
                if *bold { 10.5 } else { 8.8 },
                *bold,
                if *bold { COLOR_INK } else { COLOR_MUTED },
                line,
            );
        }
        if let Some((line, bold)) = bill_lines.get(index) {
            renderer.current.text(
                312.0,
                renderer.y,
                if *bold { 10.5 } else { 8.8 },
                *bold,
                if *bold { COLOR_INK } else { COLOR_MUTED },
                line,
            );
        }
        renderer.y -= 14.0;
    }
    renderer.y -= 18.0;
}

fn contact_lines(contact: &Contact, width: f32) -> Vec<(String, bool)> {
    let mut lines = vec![(contact.name.clone(), true)];
    if let Some(email) = contact.email.as_deref() {
        lines.extend(
            wrap_text(email, width, 8.8, false)
                .into_iter()
                .map(|line| (line, false)),
        );
    }
    if let Some(address) = contact.address.as_deref() {
        lines.extend(
            wrap_text(address, width, 8.8, false)
                .into_iter()
                .map(|line| (line, false)),
        );
    }
    lines
}

fn render_items(renderer: &mut InvoiceRenderer, invoice: &InvoiceDocument) {
    renderer.ensure_space(55.0, invoice);
    render_table_header(renderer);
    if invoice.items.is_empty() {
        renderer.current.text(
            MARGIN + 8.0,
            renderer.y - 19.0,
            9.0,
            false,
            COLOR_MUTED,
            "No line items",
        );
        renderer.y -= 34.0;
        return;
    }

    for item in &invoice.items {
        let description_lines = wrap_text(&item.description, 247.0, 9.0, false);
        let row_height = (description_lines.len() as f32 * 12.0 + 15.0).max(32.0);
        if renderer.ensure_space(row_height + 5.0, invoice) {
            renderer.current.text(
                MARGIN,
                renderer.y,
                8.0,
                true,
                COLOR_GREEN,
                "LINE ITEMS · CONTINUED",
            );
            renderer.y -= 18.0;
            render_table_header(renderer);
        }
        let baseline = renderer.y - 18.0;
        for (line_index, line) in description_lines.iter().enumerate() {
            renderer.current.text(
                MARGIN + 8.0,
                baseline - line_index as f32 * 12.0,
                9.0,
                line_index == 0,
                COLOR_INK,
                line,
            );
        }
        renderer.current.text_right(
            353.0,
            baseline,
            8.8,
            false,
            COLOR_MUTED,
            &format_quantity(item.quantity_millis),
        );
        renderer.current.text_right(
            448.0,
            baseline,
            8.8,
            false,
            COLOR_MUTED,
            &format_money(item.unit_price_minor, &invoice.header.currency),
        );
        renderer.current.text_right(
            CONTENT_RIGHT - 8.0,
            baseline,
            9.0,
            true,
            COLOR_INK,
            &format_money(item.line_total_minor, &invoice.header.currency),
        );
        renderer.current.line(
            MARGIN,
            renderer.y - row_height,
            CONTENT_RIGHT,
            renderer.y - row_height,
            0.55,
            COLOR_LINE,
        );
        renderer.y -= row_height;
    }
    renderer.y -= 12.0;
}

fn render_table_header(renderer: &mut InvoiceRenderer) {
    let height = 25.0;
    renderer.current.fill_rect(
        MARGIN,
        renderer.y - height,
        CONTENT_RIGHT - MARGIN,
        height,
        COLOR_INK,
    );
    renderer.current.text(
        MARGIN + 8.0,
        renderer.y - 17.0,
        7.5,
        true,
        COLOR_WHITE,
        "DESCRIPTION",
    );
    renderer
        .current
        .text_right(353.0, renderer.y - 17.0, 7.5, true, COLOR_WHITE, "QTY");
    renderer
        .current
        .text_right(448.0, renderer.y - 17.0, 7.5, true, COLOR_WHITE, "RATE");
    renderer.current.text_right(
        CONTENT_RIGHT - 8.0,
        renderer.y - 17.0,
        7.5,
        true,
        COLOR_WHITE,
        "AMOUNT",
    );
    renderer.y -= height;
}

fn render_totals(renderer: &mut InvoiceRenderer, invoice: &InvoiceDocument) {
    let paid_minor = invoice.payments.iter().fold(0_i64, |sum, payment| {
        sum.saturating_add(payment.amount_minor)
    });
    let credit_minor = paid_minor.saturating_sub(invoice.header.total_minor).max(0);
    let adjustment_rows = invoice.adjustments.len().max(
        usize::from(invoice.header.discount_total_minor > 0)
            + usize::from(invoice.header.tax_total_minor > 0),
    );
    let credit_height = if credit_minor > 0 { 18.0 } else { 0.0 };
    let required = 116.0 + adjustment_rows as f32 * 16.0 + credit_height;
    renderer.ensure_space(required, invoice);
    let left = 332.0;
    let right = CONTENT_RIGHT;

    summary_row(
        &mut renderer.current,
        left,
        right,
        renderer.y,
        "Subtotal",
        &format_money(invoice.header.subtotal_minor, &invoice.header.currency),
        false,
    );
    renderer.y -= 17.0;

    if invoice.adjustments.is_empty() {
        if invoice.header.discount_total_minor > 0 {
            summary_row(
                &mut renderer.current,
                left,
                right,
                renderer.y,
                "Discount",
                &format!(
                    "-{}",
                    format_money(
                        invoice.header.discount_total_minor,
                        &invoice.header.currency
                    )
                ),
                false,
            );
            renderer.y -= 17.0;
        }
        if invoice.header.tax_total_minor > 0 {
            summary_row(
                &mut renderer.current,
                left,
                right,
                renderer.y,
                "Tax",
                &format_money(invoice.header.tax_total_minor, &invoice.header.currency),
                false,
            );
            renderer.y -= 17.0;
        }
    } else {
        for adjustment in &invoice.adjustments {
            let label = adjustment_label(adjustment);
            let amount = format_money(adjustment.amount_minor, &invoice.header.currency);
            let displayed = if adjustment.kind == "discount" {
                format!("-{amount}")
            } else {
                amount
            };
            summary_row(
                &mut renderer.current,
                left,
                right,
                renderer.y,
                &label,
                &displayed,
                false,
            );
            renderer.y -= 17.0;
        }
    }

    renderer.current.line(
        left,
        renderer.y + 5.0,
        right,
        renderer.y + 5.0,
        0.7,
        COLOR_LINE,
    );
    renderer.y -= 10.0;
    summary_row(
        &mut renderer.current,
        left,
        right,
        renderer.y,
        "Total",
        &format_money(invoice.header.total_minor, &invoice.header.currency),
        true,
    );
    renderer.y -= 21.0;

    summary_row(
        &mut renderer.current,
        left,
        right,
        renderer.y,
        "Paid",
        &format_money(paid_minor, &invoice.header.currency),
        false,
    );
    renderer.y -= 26.0;

    let balance_minor = invoice.header.total_minor.saturating_sub(paid_minor).max(0);
    renderer.current.fill_rect(
        left - 8.0,
        renderer.y - 7.0,
        right - left + 16.0,
        24.0,
        COLOR_SOFT,
    );
    renderer
        .current
        .text(left, renderer.y, 9.5, true, COLOR_GREEN, "BALANCE DUE");
    renderer.current.text_right(
        right,
        renderer.y,
        11.0,
        true,
        COLOR_GREEN,
        &format_money(balance_minor, &invoice.header.currency),
    );
    renderer.y -= 37.0;
    if credit_minor > 0 {
        renderer
            .current
            .text(left, renderer.y, 9.0, true, COLOR_GREEN, "CREDIT");
        renderer.current.text_right(
            right,
            renderer.y,
            9.5,
            true,
            COLOR_GREEN,
            &format_money(credit_minor, &invoice.header.currency),
        );
        renderer.y -= 18.0;
    }
}

fn summary_row(
    canvas: &mut Canvas,
    left: f32,
    right: f32,
    y: f32,
    label: &str,
    amount: &str,
    emphasized: bool,
) {
    canvas.text(
        left,
        y,
        if emphasized { 10.5 } else { 9.0 },
        emphasized,
        if emphasized { COLOR_INK } else { COLOR_MUTED },
        label,
    );
    canvas.text_right(
        right,
        y,
        if emphasized { 11.0 } else { 9.0 },
        emphasized,
        if emphasized { COLOR_INK } else { COLOR_MUTED },
        amount,
    );
}

fn render_payments(renderer: &mut InvoiceRenderer, invoice: &InvoiceDocument) {
    if invoice.payments.is_empty() {
        return;
    }
    renderer.ensure_space(52.0, invoice);
    renderer.current.text(
        MARGIN,
        renderer.y,
        7.5,
        true,
        COLOR_GREEN,
        "PAYMENT HISTORY",
    );
    renderer.y -= 20.0;

    for payment in &invoice.payments {
        if renderer.ensure_space(34.0, invoice) {
            renderer.current.text(
                MARGIN,
                renderer.y,
                7.5,
                true,
                COLOR_GREEN,
                "PAYMENT HISTORY · CONTINUED",
            );
            renderer.y -= 20.0;
        }
        renderer.current.text(
            MARGIN,
            renderer.y,
            9.0,
            true,
            COLOR_INK,
            &display_date(&payment.paid_at),
        );
        let detail = payment_detail(payment);
        if !detail.is_empty() {
            renderer
                .current
                .text(151.0, renderer.y, 8.5, false, COLOR_MUTED, &detail);
        }
        renderer.current.text_right(
            CONTENT_RIGHT,
            renderer.y,
            9.0,
            true,
            COLOR_INK,
            &format_money(payment.amount_minor, &invoice.header.currency),
        );
        renderer.y -= 18.0;
        renderer.current.line(
            MARGIN,
            renderer.y + 6.0,
            CONTENT_RIGHT,
            renderer.y + 6.0,
            0.5,
            COLOR_LINE,
        );
    }
    renderer.y -= 14.0;
}

fn payment_detail(payment: &InvoicePayment) -> String {
    [
        payment.payment_method.as_deref(),
        payment.reference.as_deref(),
        payment.notes.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

fn render_text_section(
    renderer: &mut InvoiceRenderer,
    invoice: &InvoiceDocument,
    label: &str,
    text: &str,
) {
    let lines = wrap_text(text, CONTENT_RIGHT - MARGIN, 8.8, false);
    if lines.is_empty() {
        return;
    }
    renderer.ensure_space(44.0, invoice);
    renderer
        .current
        .text(MARGIN, renderer.y, 7.5, true, COLOR_GREEN, label);
    renderer.y -= 19.0;
    for line in lines {
        if renderer.ensure_space(14.0, invoice) {
            renderer.current.text(
                MARGIN,
                renderer.y,
                7.5,
                true,
                COLOR_GREEN,
                &format!("{label} · CONTINUED"),
            );
            renderer.y -= 19.0;
        }
        renderer
            .current
            .text(MARGIN, renderer.y, 8.8, false, COLOR_MUTED, &line);
        renderer.y -= 13.0;
    }
    renderer.y -= 16.0;
}

fn adjustment_label(adjustment: &InvoiceAdjustment) -> String {
    match (
        adjustment.calculation_type.as_str(),
        adjustment.rate_basis_points,
    ) {
        ("percentage", Some(rate)) => {
            format!("{} ({})", adjustment.label, format_percent(rate))
        }
        _ => adjustment.label.clone(),
    }
}

fn milestone_label(header: &InvoiceHeader) -> String {
    let label = header
        .milestone_label
        .as_deref()
        .unwrap_or(match header.milestone_kind.as_str() {
            "kickoff" => "Kickoff",
            "completion" => "Completion",
            _ => "Custom",
        });
    match header.milestone_percent_basis_points {
        Some(percent) => format!("{label} · {}", format_percent(percent)),
        None => label.into(),
    }
}

fn format_percent(basis_points: i64) -> String {
    let whole = basis_points / 100;
    let decimal = basis_points.unsigned_abs() % 100;
    if decimal == 0 {
        format!("{whole}%")
    } else if decimal % 10 == 0 {
        format!("{whole}.{}%", decimal / 10)
    } else {
        format!("{whole}.{decimal:02}%")
    }
}

fn format_quantity(quantity_millis: i64) -> String {
    let whole = quantity_millis / 1000;
    let fraction = quantity_millis.unsigned_abs() % 1000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn format_money(minor: i64, currency: &str) -> String {
    let negative = minor < 0;
    let absolute = minor.unsigned_abs();
    let major = absolute / 100;
    let cents = absolute % 100;
    let grouped = group_digits(major);
    format!(
        "{}{} {}.{:02}",
        if negative { "-" } else { "" },
        currency,
        grouped,
        cents
    )
}

fn group_digits(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(character);
    }
    grouped
}

fn display_status(status: &str) -> String {
    status
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_date(value: &str) -> String {
    let date_text = value.get(..10).unwrap_or(value);
    let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") else {
        return value.to_owned();
    };
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    format!(
        "{} {}, {}",
        MONTHS[date.month0() as usize],
        date.day(),
        date.year()
    )
}

fn truncate_to_width(value: &str, max_width: f32, size: f32, bold: bool) -> String {
    if text_width(value, size, bold) <= max_width {
        return value.to_owned();
    }
    let ellipsis = "...";
    let target = max_width - text_width(ellipsis, size, bold);
    let mut result = String::new();
    for character in value.chars() {
        let candidate = format!("{result}{character}");
        if text_width(&candidate, size, bold) > target {
            break;
        }
        result.push(character);
    }
    result.push_str(ellipsis);
    result
}

fn wrap_text(value: &str, max_width: f32, size: f32, bold: bool) -> Vec<String> {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = Vec::new();
    for paragraph in normalized.split('\n') {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let fragments = split_word(word, max_width, size, bold);
            for (index, fragment) in fragments.iter().enumerate() {
                let candidate = if current.is_empty() {
                    fragment.clone()
                } else {
                    format!("{current} {fragment}")
                };
                if text_width(&candidate, size, bold) <= max_width {
                    current = candidate;
                } else {
                    if !current.is_empty() {
                        lines.push(std::mem::take(&mut current));
                    }
                    current = fragment.clone();
                }
                if index + 1 < fragments.len() && !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    lines
}

fn split_word(word: &str, max_width: f32, size: f32, bold: bool) -> Vec<String> {
    if text_width(word, size, bold) <= max_width {
        return vec![word.to_owned()];
    }
    let mut fragments = Vec::new();
    let mut fragment = String::new();
    for character in word.chars() {
        let candidate = format!("{fragment}{character}");
        if !fragment.is_empty() && text_width(&candidate, size, bold) > max_width {
            fragments.push(std::mem::take(&mut fragment));
        }
        fragment.push(character);
    }
    if !fragment.is_empty() {
        fragments.push(fragment);
    }
    fragments
}

fn text_width(value: &str, size: f32, bold: bool) -> f32 {
    value
        .chars()
        .map(|character| {
            let factor = match character {
                ' ' => 0.278,
                'i' | 'l' | 'I' | '.' | ',' | ':' | ';' | '!' | '\'' | '|' => 0.24,
                'f' | 't' | 'r' | '(' | ')' | '[' | ']' => 0.34,
                'm' | 'w' | 'M' | 'W' | '@' => 0.82,
                '0'..='9' => 0.56,
                'A'..='Z' => 0.64,
                _ => 0.51,
            };
            size * factor * if bold { 1.035 } else { 1.0 }
        })
        .sum()
}

fn pdf_literal(value: &str) -> String {
    let mut result = String::from("(");
    for byte in encode_win_ansi(value) {
        match byte {
            b'(' | b')' | b'\\' => {
                result.push('\\');
                result.push(byte as char);
            }
            32..=126 => result.push(byte as char),
            _ => result.push_str(&format!("\\{byte:03o}")),
        }
    }
    result.push(')');
    result
}

fn encode_win_ansi(value: &str) -> Vec<u8> {
    value
        .chars()
        .map(|character| match character {
            '\n' | '\r' | '\t' => b' ',
            '\u{20AC}' => 0x80,
            '\u{201A}' => 0x82,
            '\u{0192}' => 0x83,
            '\u{201E}' => 0x84,
            '\u{2026}' => 0x85,
            '\u{2020}' => 0x86,
            '\u{2021}' => 0x87,
            '\u{02C6}' => 0x88,
            '\u{2030}' => 0x89,
            '\u{0160}' => 0x8A,
            '\u{2039}' => 0x8B,
            '\u{0152}' => 0x8C,
            '\u{017D}' => 0x8E,
            '\u{2018}' => 0x91,
            '\u{2019}' => 0x92,
            '\u{201C}' => 0x93,
            '\u{201D}' => 0x94,
            '\u{2022}' => 0x95,
            '\u{2013}' => 0x96,
            '\u{2014}' => 0x97,
            '\u{02DC}' => 0x98,
            '\u{2122}' => 0x99,
            '\u{0161}' => 0x9A,
            '\u{203A}' => 0x9B,
            '\u{0153}' => 0x9C,
            '\u{017E}' => 0x9E,
            '\u{0178}' => 0x9F,
            value if (value as u32) <= 0x7E && (value as u32) >= 0x20 => value as u8,
            value if (0xA0..=0xFF).contains(&(value as u32)) => value as u8,
            _ => b'?',
        })
        .collect()
}

fn build_pdf(pages: &[String]) -> Vec<u8> {
    let page_count = pages.len().max(1);
    let mut objects = vec![Vec::<u8>::new(); 4 + page_count * 2];
    objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();

    let page_ids = (0..page_count)
        .map(|index| 5 + index * 2)
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    objects[1] = format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        page_count, page_ids
    )
    .into_bytes();
    objects[2] =
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_vec();
    objects[3] =
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_vec();

    for index in 0..page_count {
        let page_object_index = 4 + index * 2;
        let page_object_id = page_object_index + 1;
        let stream_object_id = page_object_id + 1;
        objects[page_object_index] = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
             /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> \
             /Contents {} 0 R >>",
            PAGE_WIDTH, PAGE_HEIGHT, stream_object_id
        )
        .into_bytes();

        let content = pages.get(index).map(String::as_bytes).unwrap_or_default();
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(content);
        stream.extend_from_slice(b"\nendstream");
        objects[page_object_index + 1] = stream;
    }

    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len() + 1);
    offsets.push(0_usize);
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    pdf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_invoice(item_count: usize) -> InvoiceDocument {
        InvoiceDocument {
            header: InvoiceHeader {
                number: "INV-2026-07-23-0001".into(),
                status: "partially_paid".into(),
                issue_date: "2026-07-23".into(),
                due_date: "2026-08-06".into(),
                currency: "USD".into(),
                subtotal_minor: item_count as i64 * 10_000,
                discount_total_minor: 1_000,
                tax_total_minor: 990,
                total_minor: item_count as i64 * 10_000 - 10,
                notes: Some("Thank you for your business.".into()),
                payment_instructions: Some("Wire payment using the invoice number.".into()),
                seller: Contact {
                    name: "RudeSync Studio".into(),
                    email: Some("hello@example.com".into()),
                    address: Some("123 Developer Avenue\nMakati City".into()),
                },
                bill_to: Contact {
                    name: "Example Client".into(),
                    email: Some("billing@client.test".into()),
                    address: Some("45 Client Road\nNew York, NY".into()),
                },
                project_name: Some("Client Portal".into()),
                milestone_kind: "kickoff".into(),
                milestone_percent_basis_points: Some(5000),
                milestone_label: Some("Project kickoff".into()),
            },
            items: (0..item_count)
                .map(|index| InvoiceItem {
                    description: format!(
                        "Milestone deliverable {} with implementation and handoff documentation",
                        index + 1
                    ),
                    quantity_millis: 1000,
                    unit_price_minor: 10_000,
                    line_total_minor: 10_000,
                })
                .collect(),
            adjustments: vec![
                InvoiceAdjustment {
                    kind: "discount".into(),
                    label: "Project discount".into(),
                    calculation_type: "fixed".into(),
                    rate_basis_points: None,
                    amount_minor: 1_000,
                },
                InvoiceAdjustment {
                    kind: "tax".into(),
                    label: "Tax".into(),
                    calculation_type: "percentage".into(),
                    rate_basis_points: Some(1000),
                    amount_minor: 990,
                },
            ],
            payments: vec![InvoicePayment {
                amount_minor: 25_000,
                paid_at: "2026-07-24".into(),
                payment_method: Some("Bank transfer".into()),
                reference: Some("REF-123".into()),
                notes: None,
            }],
        }
    }

    #[test]
    fn pdf_strings_escape_delimiters_and_control_text() {
        let escaped = pdf_literal("Client (North) \\ phase\n“one”");
        assert!(escaped.starts_with('(') && escaped.ends_with(')'));
        assert!(escaped.contains("\\(North\\)"));
        assert!(escaped.contains("\\\\"));
        assert!(!escaped.contains('\n'));
        assert!(escaped.contains("\\223one\\224"));
    }

    #[test]
    fn wrapping_keeps_even_long_tokens_inside_the_column() {
        let lines = wrap_text(
            "Normal words abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
            80.0,
            9.0,
            false,
        );
        assert!(lines.len() > 2);
        assert!(lines
            .iter()
            .all(|line| text_width(line, 9.0, false) <= 80.1));
    }

    #[test]
    fn renderer_paginates_large_invoices_and_writes_valid_xref() {
        let invoice = sample_invoice(80);
        let pages = render_invoice(&invoice);
        assert!(pages.len() > 1);
        assert!(pages.iter().skip(1).any(|page| page.contains("LINE ITEMS")));
        let pdf = build_pdf(&pages);
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.windows(4).any(|window| window == b"xref"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert!(String::from_utf8_lossy(&pdf).contains(&format!("/Count {}", pages.len())));
    }

    #[test]
    fn renderer_displays_invoice_credit_after_an_overpayment() {
        let invoice = sample_invoice(1);
        let pages = render_invoice(&invoice);
        assert!(pages.iter().any(|page| page.contains("(CREDIT)")));
        assert!(pages.iter().any(|page| page.contains("(USD 150.10)")));
    }

    #[test]
    fn money_dates_and_quantities_are_human_readable() {
        assert_eq!(display_date("2026-07-23"), "July 23, 2026");
        assert_eq!(format_money(123_456, "USD"), "USD 1,234.56");
        assert_eq!(format_quantity(1500), "1.5");
        assert_eq!(format_percent(5050), "50.5%");
    }

    #[test]
    fn destination_requires_pdf_and_existing_parent() {
        let parent = std::env::temp_dir();
        let valid = parent.join("rudesync-invoice-test.pdf");
        assert!(validate_destination(valid.to_string_lossy().as_ref()).is_ok());
        let invalid = parent.join("rudesync-invoice-test.txt");
        assert!(validate_destination(invalid.to_string_lossy().as_ref()).is_err());
        let missing = parent
            .join("rudesync-folder-that-does-not-exist")
            .join("invoice.pdf");
        assert!(validate_destination(missing.to_string_lossy().as_ref()).is_err());
    }
}
