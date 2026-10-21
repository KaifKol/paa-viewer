#pragma once

#include <QMainWindow>
#include <QGraphicsView>
#include <QGraphicsScene>
#include <QGraphicsPixmapItem>
#include <QTableWidget>
#include <QLabel>
#include <QComboBox>
#include <QPushButton>
#include <QDockWidget>
#include <QImage>
#include <QVector>

#include "paa_ffi.h"

enum class ChannelMode {
    Rgba,
    Rgb,
    Red,
    Green,
    Blue,
    Alpha
};

class TextureCanvas : public QGraphicsView {
    Q_OBJECT
public:
    explicit TextureCanvas(QWidget* parent = nullptr);
    void setImage(const QImage& img, ChannelMode mode);
    void resetZoom();
    void fitZoom();

signals:
    void pixelHovered(int x, int y, QColor color);
    void pixelLeft();

protected:
    void wheelEvent(QWheelEvent* event) override;
    void mouseMoveEvent(QMouseEvent* event) override;
    void leaveEvent(QEvent* event) override;
    void drawBackground(QPainter* painter, const QRectF& rect) override;

private:
    QGraphicsScene* m_scene;
    QGraphicsPixmapItem* m_pixmapItem;
    QImage m_sourceImage;
    double m_zoomFactor;
};

class MainWindow : public QMainWindow {
    Q_OBJECT

public:
    explicit MainWindow(const QString& initialFile = QString(), QWidget* parent = nullptr);
    ~MainWindow() override;

    void loadFile(const QString& filePath);

protected:
    void dragEnterEvent(QDragEnterEvent* event) override;
    void dropEvent(QDropEvent* event) override;

private slots:
    void onOpenFile();
    void onExportFile();
    void onMipmapSelected(int row, int column);
    void onChannelChanged(int index);
    void onQuickScaleHalf();
    void onQuickScaleDouble();
    void onQuickPotNext();
    void onToggleContextMenu();
    void onExportWindowsReg();
    void onPixelHovered(int x, int y, QColor color);
    void onPixelLeft();

private:
    void setupUi();
    void setupMenus();
    void setupDock();
    void updateTextureView();
    void updateMetadataDisplay();
    void updateContextMenuButton();

    TextureCanvas* m_canvas;
    QDockWidget* m_dock;
    QTableWidget* m_mipmapTable;
    QLabel* m_formatLabel;
    QLabel* m_dimsLabel;
    QLabel* m_potLabel;
    QLabel* m_aspectLabel;
    QLabel* m_fileSizeLabel;
    QLabel* m_avgColorSwatch;
    QLabel* m_maxColorSwatch;
    QLabel* m_cursorLabel;
    QLabel* m_pixelColorSwatch;
    QPushButton* m_contextMenuBtn;
    QComboBox* m_channelCombo;

    QString m_currentFilePath;
    PaaFfiHandle* m_handle;
    uint32_t m_currentMipLevel;
    QImage m_currentImage;
    ChannelMode m_channelMode;
};
