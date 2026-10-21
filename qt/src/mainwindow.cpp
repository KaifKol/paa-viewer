#include "mainwindow.h"

#include <QApplication>
#include <QMenuBar>
#include <QToolBar>
#include <QStatusBar>
#include <QFileDialog>
#include <QMessageBox>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QMimeData>
#include <QHeaderView>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGroupBox>
#include <QPainter>
#include <QWheelEvent>
#include <QMouseEvent>
#include <QFileInfo>
#include <QProcess>

// =============================================================================
// TextureCanvas Implementation
// =============================================================================

TextureCanvas::TextureCanvas(QWidget* parent)
    : QGraphicsView(parent)
    , m_scene(new QGraphicsScene(this))
    , m_pixmapItem(new QGraphicsPixmapItem())
    , m_zoomFactor(1.0)
{
    setScene(m_scene);
    m_scene->addItem(m_pixmapItem);
    setDragMode(QGraphicsView::ScrollHandDrag);
    setTransformationAnchor(QGraphicsView::AnchorUnderMouse);
    setResizeAnchor(QGraphicsView::AnchorUnderMouse);
    setMouseTracking(true);
    setRenderHint(QPainter::SmoothPixmapTransform, false);
}

void TextureCanvas::setImage(const QImage& img, ChannelMode mode) {
    m_sourceImage = img;
    if (m_sourceImage.isNull()) {
        m_pixmapItem->setPixmap(QPixmap());
        return;
    }

    QImage displayImg = m_sourceImage;
    if (mode != ChannelMode::Rgba) {
        displayImg = displayImg.convertToFormat(QImage::Format_RGBA8888);
        int w = displayImg.width();
        int h = displayImg.height();
        for (int y = 0; y < h; ++y) {
            QRgb* line = reinterpret_cast<QRgb*>(displayImg.scanLine(y));
            for (int x = 0; x < w; ++x) {
                QRgb p = line[x];
                int r = qRed(p);
                int g = qGreen(p);
                int b = qBlue(p);
                int a = qAlpha(p);

                switch (mode) {
                    case ChannelMode::Rgb:
                        line[x] = qRgba(r, g, b, 255);
                        break;
                    case ChannelMode::Red:
                        line[x] = qRgba(r, 0, 0, 255);
                        break;
                    case ChannelMode::Green:
                        line[x] = qRgba(0, g, 0, 255);
                        break;
                    case ChannelMode::Blue:
                        line[x] = qRgba(0, 0, b, 255);
                        break;
                    case ChannelMode::Alpha:
                        line[x] = qRgba(a, a, a, 255);
                        break;
                    default:
                        break;
                }
            }
        }
    }

    m_pixmapItem->setPixmap(QPixmap::fromImage(displayImg));
    m_scene->setSceneRect(m_pixmapItem->boundingRect());
}

void TextureCanvas::resetZoom() {
    resetTransform();
    m_zoomFactor = 1.0;
}

void TextureCanvas::fitZoom() {
    if (m_pixmapItem->pixmap().isNull()) return;
    fitInView(m_pixmapItem, Qt::KeepAspectRatio);
}

void TextureCanvas::wheelEvent(QWheelEvent* event) {
    double factor = (event->angleDelta().y() > 0) ? 1.15 : 0.85;
    m_zoomFactor *= factor;
    scale(factor, factor);
    event->accept();
}

void TextureCanvas::mouseMoveEvent(QMouseEvent* event) {
    QGraphicsView::mouseMoveEvent(event);

    if (m_sourceImage.isNull()) {
        emit pixelLeft();
        return;
    }

    QPointF scenePos = mapToScene(event->pos());
    int px = static_cast<int>(scenePos.x());
    int py = static_cast<int>(scenePos.y());

    if (px >= 0 && px < m_sourceImage.width() && py >= 0 && py < m_sourceImage.height()) {
        emit pixelHovered(px, py, m_sourceImage.pixelColor(px, py));
    } else {
        emit pixelLeft();
    }
}

void TextureCanvas::leaveEvent(QEvent* event) {
    QGraphicsView::leaveEvent(event);
    emit pixelLeft();
}

void TextureCanvas::drawBackground(QPainter* painter, const QRectF& rect) {
    painter->fillRect(rect, QColor(30, 30, 46));

    if (m_pixmapItem->pixmap().isNull()) return;

    QRectF imgRect = m_pixmapItem->boundingRect();
    QRectF intersect = rect.intersected(imgRect);
    if (intersect.isEmpty()) return;

    const double checkSize = 16.0;
    QColor c1(40, 40, 55);
    QColor c2(60, 60, 80);

    int startX = static_cast<int>(std::floor(intersect.left() / checkSize));
    int endX = static_cast<int>(std::ceil(intersect.right() / checkSize));
    int startY = static_cast<int>(std::floor(intersect.top() / checkSize));
    int endY = static_cast<int>(std::ceil(intersect.bottom() / checkSize));

    for (int y = startY; y < endY; ++y) {
        for (int x = startX; x < endX; ++x) {
            QRectF cell(x * checkSize, y * checkSize, checkSize, checkSize);
            painter->fillRect(cell.intersected(imgRect), ((x + y) % 2 == 0) ? c1 : c2);
        }
    }
}

// =============================================================================
// MainWindow Implementation
// =============================================================================

MainWindow::MainWindow(const QString& initialFile, QWidget* parent)
    : QMainWindow(parent)
    , m_canvas(new TextureCanvas(this))
    , m_handle(nullptr)
    , m_currentMipLevel(0)
    , m_channelMode(ChannelMode::Rgba)
{
    setupUi();
    setupMenus();
    setupDock();

    connect(m_canvas, &TextureCanvas::pixelHovered, this, &MainWindow::onPixelHovered);
    connect(m_canvas, &TextureCanvas::pixelLeft, this, &MainWindow::onPixelLeft);

    setAcceptDrops(true);
    resize(1280, 800);

    if (!initialFile.isEmpty()) {
        loadFile(initialFile);
    }
}

MainWindow::~MainWindow() {
    if (m_handle) {
        paa_ffi_free(m_handle);
        m_handle = nullptr;
    }
}

void MainWindow::setupUi() {
    setCentralWidget(m_canvas);

    // Status bar setup
    QStatusBar* status = statusBar();
    m_cursorLabel = new QLabel(this);
    m_pixelColorSwatch = new QLabel(this);
    m_pixelColorSwatch->setFixedSize(16, 16);
    m_pixelColorSwatch->setStyleSheet("border: 1px solid gray; background-color: transparent;");

    status->addWidget(m_pixelColorSwatch);
    status->addWidget(m_cursorLabel);
    status->showMessage(tr("Ready. Open a PAA or Image file, or drag and drop here."));
}

void MainWindow::setupMenus() {
    QMenuBar* mb = menuBar();

    // File Menu
    QMenu* fileMenu = mb->addMenu(tr("&File"));
    QAction* openAct = fileMenu->addAction(tr("&Open File..."), this, &MainWindow::onOpenFile);
    openAct->setShortcut(QKeySequence::Open);

    QAction* exportAct = fileMenu->addAction(tr("&Export / Save As..."), this, &MainWindow::onExportFile);
    exportAct->setShortcut(QKeySequence::SaveAs);

    fileMenu->addSeparator();
    fileMenu->addAction(tr("E&xit"), QKeySequence::Quit, this, &QWidget::close);

    // View Menu
    QMenu* viewMenu = mb->addMenu(tr("&View"));
    viewMenu->addAction(tr("Reset Zoom (1:1)"), m_canvas, &TextureCanvas::resetZoom);
    viewMenu->addAction(tr("Fit to Window"), m_canvas, &TextureCanvas::fitZoom);

    // Tools Menu
    QMenu* toolsMenu = mb->addMenu(tr("&Tools"));
    toolsMenu->addAction(tr("Halve Resolution (÷2)"), this, &MainWindow::onQuickScaleHalf);
    toolsMenu->addAction(tr("Double Resolution (x2)"), this, &MainWindow::onQuickScaleDouble);
    toolsMenu->addAction(tr("Align to Next POT"), this, &MainWindow::onQuickPotNext);
    toolsMenu->addSeparator();
    toolsMenu->addAction(tr("Export Windows .reg File"), this, &MainWindow::onExportWindowsReg);

    // Toolbar
    QToolBar* tb = addToolBar(tr("Main Toolbar"));
    tb->addAction(openAct);
    tb->addAction(exportAct);
    tb->addSeparator();

    tb->addAction(tr("1:1"), m_canvas, &TextureCanvas::resetZoom);
    tb->addAction(tr("Fit"), m_canvas, &TextureCanvas::fitZoom);
    tb->addSeparator();

    tb->addWidget(new QLabel(tr(" Channel: "), this));
    m_channelCombo = new QComboBox(this);
    m_channelCombo->addItems({tr("RGBA"), tr("RGB"), tr("Red"), tr("Green"), tr("Blue"), tr("Alpha")});
    connect(m_channelCombo, QOverload<int>::of(&QComboBox::currentIndexChanged), this, &MainWindow::onChannelChanged);
    tb->addWidget(m_channelCombo);

    tb->addSeparator();
    QPushButton* btnHalf = new QPushButton(tr("÷2 (x0.5)"), this);
    connect(btnHalf, &QPushButton::clicked, this, &MainWindow::onQuickScaleHalf);
    tb->addWidget(btnHalf);

    QPushButton* btnDouble = new QPushButton(tr("x2"), this);
    connect(btnDouble, &QPushButton::clicked, this, &MainWindow::onQuickScaleDouble);
    tb->addWidget(btnDouble);
}

void MainWindow::setupDock() {
    m_dock = new QDockWidget(tr("Texture Inspector"), this);
    m_dock->setAllowedAreas(Qt::LeftDockWidgetArea | Qt::RightDockWidgetArea);

    QWidget* dockContents = new QWidget(m_dock);
    QVBoxLayout* layout = new QVBoxLayout(dockContents);

    // Info Group
    QGroupBox* infoGroup = new QGroupBox(tr("Properties"), dockContents);
    QVBoxLayout* infoLayout = new QVBoxLayout(infoGroup);

    m_formatLabel = new QLabel(tr("Format: None"), infoGroup);
    m_dimsLabel = new QLabel(tr("Dimensions: -"), infoGroup);
    m_aspectLabel = new QLabel(tr("Aspect Ratio: -"), infoGroup);
    m_potLabel = new QLabel(tr("Power of Two: -"), infoGroup);
    m_fileSizeLabel = new QLabel(tr("File Size: -"), infoGroup);

    infoLayout->addWidget(m_formatLabel);
    infoLayout->addWidget(m_dimsLabel);
    infoLayout->addWidget(m_aspectLabel);
    infoLayout->addWidget(m_potLabel);
    infoLayout->addWidget(m_fileSizeLabel);
    layout->addWidget(infoGroup);

    // Colors Group
    QGroupBox* colorGroup = new QGroupBox(tr("Color Tags"), dockContents);
    QHBoxLayout* colorLayout = new QHBoxLayout(colorGroup);

    m_avgColorSwatch = new QLabel(this);
    m_avgColorSwatch->setFixedSize(24, 24);
    m_avgColorSwatch->setStyleSheet("border: 1px solid white; background-color: #000; border-radius: 4px;");

    m_maxColorSwatch = new QLabel(this);
    m_maxColorSwatch->setFixedSize(24, 24);
    m_maxColorSwatch->setStyleSheet("border: 1px solid white; background-color: #fff; border-radius: 4px;");

    colorLayout->addWidget(new QLabel(tr("Avg:"), this));
    colorLayout->addWidget(m_avgColorSwatch);
    colorLayout->addSpacing(16);
    colorLayout->addWidget(new QLabel(tr("Max:"), this));
    colorLayout->addWidget(m_maxColorSwatch);
    colorLayout->addStretch();
    layout->addWidget(colorGroup);

    // Mipmaps Table
    QGroupBox* mipGroup = new QGroupBox(tr("Mipmap Levels"), dockContents);
    QVBoxLayout* mipLayout = new QVBoxLayout(mipGroup);

    m_mipmapTable = new QTableWidget(0, 4, mipGroup);
    m_mipmapTable->setHorizontalHeaderLabels({tr("Level"), tr("Resolution"), tr("Data Size"), tr("Ratio")});
    m_mipmapTable->horizontalHeader()->setSectionResizeMode(QHeaderView::Stretch);
    m_mipmapTable->setSelectionBehavior(QAbstractItemView::SelectRows);
    m_mipmapTable->setSelectionMode(QAbstractItemView::SingleSelection);
    connect(m_mipmapTable, &QTableWidget::cellClicked, this, &MainWindow::onMipmapSelected);
    mipLayout->addWidget(m_mipmapTable);
    layout->addWidget(mipGroup);

    // System Integration Group
    QGroupBox* sysGroup = new QGroupBox(tr("System Integration"), dockContents);
    QVBoxLayout* sysLayout = new QVBoxLayout(sysGroup);
    m_contextMenuBtn = new QPushButton(tr("Register in Context Menu"), sysGroup);
    connect(m_contextMenuBtn, &QPushButton::clicked, this, &MainWindow::onToggleContextMenu);
    sysLayout->addWidget(m_contextMenuBtn);

    QPushButton* exportRegBtn = new QPushButton(tr("Export Windows .reg"), sysGroup);
    connect(exportRegBtn, &QPushButton::clicked, this, &MainWindow::onExportWindowsReg);
    sysLayout->addWidget(exportRegBtn);
    layout->addWidget(sysGroup);

    layout->addStretch();
    m_dock->setWidget(dockContents);
    addDockWidget(Qt::RightDockWidgetArea, m_dock);

    updateContextMenuButton();
}

void MainWindow::loadFile(const QString& filePath) {
    if (m_handle) {
        paa_ffi_free(m_handle);
        m_handle = nullptr;
    }

    m_handle = paa_ffi_load(filePath.toUtf8().constData());
    if (!m_handle) {
        QMessageBox::critical(this, tr("Error"), tr("Failed to open or parse texture file: %1").arg(filePath));
        return;
    }

    m_currentFilePath = filePath;
    m_currentMipLevel = 0;

    updateMetadataDisplay();
    updateTextureView();

    setWindowTitle(tr("PAA Viewer (Qt6) - %1").arg(QFileInfo(filePath).fileName()));
    statusBar()->showMessage(tr("Loaded: %1").arg(filePath), 4000);
}

void MainWindow::updateMetadataDisplay() {
    if (!m_handle) return;

    char* fmt = paa_ffi_get_format_name(m_handle);
    QString fmtName = QString::fromUtf8(fmt);
    paa_ffi_free_string(fmt);

    uint32_t w = paa_ffi_get_width(m_handle);
    uint32_t h = paa_ffi_get_height(m_handle);
    bool pot = paa_ffi_is_pot(m_handle);

    m_formatLabel->setText(tr("Format: <b>%1</b>").arg(fmtName));
    m_dimsLabel->setText(tr("Dimensions: <b>%1 x %2</b>").arg(w).arg(h));
    m_aspectLabel->setText(tr("Aspect Ratio: <b>%1:1</b>").arg(h > 0 ? QString::number(static_cast<double>(w) / h, 'f', 2) : "0"));

    if (pot) {
        m_potLabel->setText(tr("Power of Two: <font color='#00cc66'><b>Yes (POT compatible)</b></font>"));
    } else {
        m_potLabel->setText(tr("Power of Two: <font color='#ffaa00'><b>Warning: Non-POT</b></font>"));
    }

    QFileInfo fi(m_currentFilePath);
    if (fi.exists()) {
        m_fileSizeLabel->setText(tr("File Size: <b>%1 KB</b>").arg(fi.size() / 1024.0, 0, 'f', 1));
    }

    // Colors
    uint8_t avg[4] = {0};
    uint8_t max[4] = {255, 255, 255, 255};
    bool alpha = false;
    if (paa_ffi_get_colors(m_handle, avg, max, &alpha)) {
        m_avgColorSwatch->setStyleSheet(QString("border: 1px solid white; border-radius: 4px; background-color: rgba(%1, %2, %3, %4);")
            .arg(avg[0]).arg(avg[1]).arg(avg[2]).arg(avg[3]));
        m_maxColorSwatch->setStyleSheet(QString("border: 1px solid white; border-radius: 4px; background-color: rgba(%1, %2, %3, %4);")
            .arg(max[0]).arg(max[1]).arg(max[2]).arg(max[3]));
    }

    // Mipmaps Table
    uint32_t count = paa_ffi_get_mipmap_count(m_handle);
    m_mipmapTable->setRowCount(count);

    for (uint32_t i = 0; i < count; ++i) {
        uint32_t mw = 0, mh = 0;
        size_t comp = 0, uncomp = 0;
        bool isComp = false;
        paa_ffi_get_mipmap_info(m_handle, i, &mw, &mh, &comp, &uncomp, &isComp);

        double ratio = (uncomp > 0) ? (static_cast<double>(comp) / uncomp) * 100.0 : 100.0;
        QString resStr = QString("%1x%2").arg(mw).arg(mh);
        QString compStr = isComp ? QString("%1 (LZO)").arg(comp) : QString::number(comp);
        QString ratioStr = QString("%1%").arg(ratio, 0, 'f', 0);

        m_mipmapTable->setItem(i, 0, new QTableWidgetItem(QString::number(i)));
        m_mipmapTable->setItem(i, 1, new QTableWidgetItem(resStr));
        m_mipmapTable->setItem(i, 2, new QTableWidgetItem(compStr));
        m_mipmapTable->setItem(i, 3, new QTableWidgetItem(ratioStr));
    }

    if (count > 0) {
        m_mipmapTable->selectRow(0);
    }
}

void MainWindow::updateTextureView() {
    if (!m_handle) return;

    uint32_t mw = 0, mh = 0;
    size_t comp = 0, uncomp = 0;
    bool isComp = false;
    if (!paa_ffi_get_mipmap_info(m_handle, m_currentMipLevel, &mw, &mh, &comp, &uncomp, &isComp)) {
        return;
    }

    QByteArray buffer(mw * mh * 4, 0);
    if (!paa_ffi_decode_mipmap_rgba(m_handle, m_currentMipLevel, reinterpret_cast<uint8_t*>(buffer.data()), buffer.size())) {
        statusBar()->showMessage(tr("Failed to decode mipmap level %1").arg(m_currentMipLevel), 4000);
        return;
    }

    m_currentImage = QImage(reinterpret_cast<const uchar*>(buffer.constData()), mw, mh, QImage::Format_RGBA8888).copy();
    m_canvas->setImage(m_currentImage, m_channelMode);
}

void MainWindow::onOpenFile() {
    QString filter = tr("All Supported Files (*.paa *.pac *.png *.jpg *.jpeg *.tga *.bmp *.webp);;PAA Textures (*.paa *.pac);;PNG Images (*.png);;All Files (*)");
    QString path = QFileDialog::getOpenFileName(this, tr("Open Texture or Image"), QString(), filter);
    if (!path.isEmpty()) {
        loadFile(path);
    }
}

void MainWindow::onExportFile() {
    if (m_currentImage.isNull()) return;

    QString filter = tr("PAA Texture (*.paa);;PNG Image (*.png);;JPEG Image (*.jpg);;TGA Image (*.tga);;WebP Image (*.webp)");
    QString path = QFileDialog::getSaveFileName(this, tr("Export Image / PAA"), QFileInfo(m_currentFilePath).completeBaseName(), filter);
    if (path.isEmpty()) return;

    if (path.endsWith(".paa", Qt::CaseInsensitive) || path.endsWith(".pac", Qt::CaseInsensitive)) {
        char* err = paa_ffi_convert(
            m_currentFilePath.toUtf8().constData(),
            path.toUtf8().constData(),
            "dxt5",
            nullptr,
            true
        );
        if (err) {
            QMessageBox::critical(this, tr("Export Error"), QString::fromUtf8(err));
            paa_ffi_free_string(err);
        } else {
            QMessageBox::information(this, tr("Success"), tr("Exported PAA successfully to %1").arg(path));
        }
    } else {
        if (m_currentImage.save(path)) {
            QMessageBox::information(this, tr("Success"), tr("Saved image successfully to %1").arg(path));
        } else {
            QMessageBox::critical(this, tr("Export Error"), tr("Failed to save image to %1").arg(path));
        }
    }
}

void MainWindow::onMipmapSelected(int row, int /*column*/) {
    if (row >= 0) {
        m_currentMipLevel = static_cast<uint32_t>(row);
        updateTextureView();
    }
}

void MainWindow::onChannelChanged(int index) {
    m_channelMode = static_cast<ChannelMode>(index);
    m_canvas->setImage(m_currentImage, m_channelMode);
}

void MainWindow::onQuickScaleHalf() {
    if (m_currentFilePath.isEmpty()) return;
    QString outPath = m_currentFilePath + ".half.paa";
    char* err = paa_ffi_convert(
        m_currentFilePath.toUtf8().constData(),
        outPath.toUtf8().constData(),
        nullptr,
        "0.5",
        true
    );
    if (err) {
        QMessageBox::critical(this, tr("Scale Error"), QString::fromUtf8(err));
        paa_ffi_free_string(err);
    } else {
        loadFile(outPath);
    }
}

void MainWindow::onQuickScaleDouble() {
    if (m_currentFilePath.isEmpty()) return;
    QString outPath = m_currentFilePath + ".double.paa";
    char* err = paa_ffi_convert(
        m_currentFilePath.toUtf8().constData(),
        outPath.toUtf8().constData(),
        nullptr,
        "2.0",
        true
    );
    if (err) {
        QMessageBox::critical(this, tr("Scale Error"), QString::fromUtf8(err));
        paa_ffi_free_string(err);
    } else {
        loadFile(outPath);
    }
}

void MainWindow::onQuickPotNext() {
    if (m_currentFilePath.isEmpty()) return;
    QString outPath = m_currentFilePath + ".nextpot.paa";
    char* err = paa_ffi_convert(
        m_currentFilePath.toUtf8().constData(),
        outPath.toUtf8().constData(),
        nullptr,
        "pot-next",
        true
    );
    if (err) {
        QMessageBox::critical(this, tr("POT Error"), QString::fromUtf8(err));
        paa_ffi_free_string(err);
    } else {
        loadFile(outPath);
    }
}

void MainWindow::updateContextMenuButton() {
    QString appDir = QApplication::applicationDirPath();
    QString exePath = appDir + "/paa-viewer";

    QProcess proc;
    proc.start(exePath, {"context-menu", "status"});
    proc.waitForFinished();
    QString out = QString::fromUtf8(proc.readAllStandardOutput());

    if (out.contains("is currently registered", Qt::CaseInsensitive)) {
        m_contextMenuBtn->setText(tr("Remove from Context Menu"));
    } else {
        m_contextMenuBtn->setText(tr("Register in Context Menu"));
    }
}

void MainWindow::onToggleContextMenu() {
    QString appDir = QApplication::applicationDirPath();
    QString exePath = appDir + "/paa-viewer";

    bool isInstall = m_contextMenuBtn->text().contains("Register");
    QString action = isInstall ? "install" : "uninstall";

    QProcess proc;
    proc.start(exePath, {"context-menu", action});
    proc.waitForFinished();

    QString out = QString::fromUtf8(proc.readAllStandardOutput());
    QMessageBox::information(this, tr("Context Menu Integration"), out);
    updateContextMenuButton();
}

void MainWindow::onExportWindowsReg() {
    QString path = QFileDialog::getSaveFileName(this, tr("Export Windows Registry File"), "paa-viewer-context-menu.reg", tr("Registry Script (*.reg)"));
    if (path.isEmpty()) return;

    QString appDir = QApplication::applicationDirPath();
    QString exePath = appDir + "/paa-viewer";

    QProcess proc;
    proc.start(exePath, {"context-menu", "export-reg", path});
    proc.waitForFinished();

    QMessageBox::information(this, tr("Windows Registry Export"), tr("Exported Windows .reg file to: %1").arg(path));
}

void MainWindow::onPixelHovered(int x, int y, QColor color) {
    m_cursorLabel->setText(tr("Cursor: (%1, %2) | RGBA: [%3, %4, %5, %6] | %7")
        .arg(x).arg(y)
        .arg(color.red()).arg(color.green()).arg(color.blue()).arg(color.alpha())
        .arg(color.name(QColor::HexArgb).toUpper()));

    m_pixelColorSwatch->setStyleSheet(QString("border: 1px solid gray; border-radius: 2px; background-color: rgba(%1, %2, %3, %4);")
        .arg(color.red()).arg(color.green()).arg(color.blue()).arg(color.alpha()));
}

void MainWindow::onPixelLeft() {
    m_cursorLabel->setText(QString());
    m_pixelColorSwatch->setStyleSheet("border: 1px solid gray; background-color: transparent;");
}

void MainWindow::dragEnterEvent(QDragEnterEvent* event) {
    if (event->mimeData()->hasUrls()) {
        event->acceptProposedAction();
    }
}

void MainWindow::dropEvent(QDropEvent* event) {
    const QList<QUrl> urls = event->mimeData()->urls();
    if (!urls.isEmpty()) {
        loadFile(urls.first().toLocalFile());
    }
}
