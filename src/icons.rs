use eframe::egui::{self, Color32, FontFamily, FontId, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Icon {
    Home,
    Desktop,
    Download,
    Document,
    Picture,
    Music,
    Video,
    Drive,
    Mount,
    Removable,
    Edit,
    Settings,
    NewFolder,
    Copy,
    Cut,
    Paste,
    More,
    Hidden,
    Details,
    List,
    Grid,
    Sun,
    Moon,
    Folder,
    File,
    Link,
    Back,
    Forward,
    Up,
    Refresh,
    Open,
    NewWindow,
    NewTab,
    DuplicateTab,
    CloseOtherTabs,
    CloseRightTabs,
    Terminal,
    OpenWith,
    Rename,
    CopyPath,
    Properties,
    Pin,
    Unpin,
    Network,
    Trash,
    DeletePermanently,
    CancelEdit,
    BrowseFolders,
    ZoomIn,
    ZoomOut,
    TrashBin,
    Restore,
    EmptyTrash,
    FilePdf,
    FileDocument,
    FileSpreadsheet,
    FilePresentation,
    FileArchive,
    FileImage,
    FileAudio,
    FileVideo,
    FileCode,
    FileText,
    FilePackage,
    FileExecutable,
    FileCertificate,
    FileDatabase,
    FileDisk,
    FileDiagram,
    FileFont,
    FileBinary,
    FileBackup,
    FileCalendar,
    NoticeSuccess,
    NoticeWarning,
    NoticeError,
}

impl Icon {
    pub const ALL: [Self; 76] = [
        Self::Home,
        Self::Desktop,
        Self::Download,
        Self::Document,
        Self::Picture,
        Self::Music,
        Self::Video,
        Self::Drive,
        Self::Mount,
        Self::Removable,
        Self::Back,
        Self::Forward,
        Self::Up,
        Self::Refresh,
        Self::Edit,
        Self::NewFolder,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::More,
        Self::Hidden,
        Self::Details,
        Self::List,
        Self::Grid,
        Self::Settings,
        Self::Sun,
        Self::Moon,
        Self::Folder,
        Self::File,
        Self::Link,
        Self::Open,
        Self::NewWindow,
        Self::NewTab,
        Self::DuplicateTab,
        Self::CloseOtherTabs,
        Self::CloseRightTabs,
        Self::Terminal,
        Self::OpenWith,
        Self::Rename,
        Self::CopyPath,
        Self::Properties,
        Self::Pin,
        Self::Unpin,
        Self::Network,
        Self::Trash,
        Self::DeletePermanently,
        Self::CancelEdit,
        Self::BrowseFolders,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::TrashBin,
        Self::Restore,
        Self::EmptyTrash,
        Self::FilePdf,
        Self::FileDocument,
        Self::FileSpreadsheet,
        Self::FilePresentation,
        Self::FileArchive,
        Self::FileImage,
        Self::FileAudio,
        Self::FileVideo,
        Self::FileCode,
        Self::FileText,
        Self::FilePackage,
        Self::FileExecutable,
        Self::FileCertificate,
        Self::FileDatabase,
        Self::FileDisk,
        Self::FileDiagram,
        Self::FileFont,
        Self::FileBinary,
        Self::FileBackup,
        Self::FileCalendar,
        Self::NoticeSuccess,
        Self::NoticeWarning,
        Self::NoticeError,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Home => crate::i18n::tr("Pasta pessoal"),
            Self::Desktop => crate::i18n::tr("Área de trabalho"),
            Self::Download => crate::i18n::tr("Downloads"),
            Self::Document => crate::i18n::tr("Documentos"),
            Self::Picture => crate::i18n::tr("Imagens"),
            Self::Music => crate::i18n::tr("Músicas"),
            Self::Video => crate::i18n::tr("Vídeos"),
            Self::Drive => crate::i18n::tr("Sistema de arquivos"),
            Self::Mount => crate::i18n::tr("Montagens"),
            Self::Removable => crate::i18n::tr("Mídias removíveis"),
            Self::Edit => crate::i18n::tr("Editar caminho"),
            Self::Settings => crate::i18n::tr("Configurações"),
            Self::NewFolder => crate::i18n::tr("Nova pasta"),
            Self::Copy => crate::i18n::tr("Copiar arquivos"),
            Self::Cut => crate::i18n::tr("Recortar arquivos"),
            Self::Paste => crate::i18n::tr("Colar arquivos"),
            Self::More => crate::i18n::tr("Mais ações da pasta"),
            Self::Hidden => crate::i18n::tr("Itens ocultos"),
            Self::Details => crate::i18n::tr("Modo de detalhes"),
            Self::List => crate::i18n::tr("Modo de lista"),
            Self::Grid => crate::i18n::tr("Modo de ícones"),
            Self::Sun => crate::i18n::tr("Tema claro"),
            Self::Moon => crate::i18n::tr("Tema escuro"),
            Self::Folder => crate::i18n::tr("Pastas na lista"),
            Self::File => crate::i18n::tr("Arquivos na lista"),
            Self::Link => crate::i18n::tr("Link simbólico"),
            Self::Back => crate::i18n::tr("Voltar"),
            Self::Forward => crate::i18n::tr("Avançar"),
            Self::Up => crate::i18n::tr("Pasta acima"),
            Self::Refresh => crate::i18n::tr("Atualizar"),
            Self::Open => crate::i18n::tr("Abrir item"),
            Self::NewWindow => crate::i18n::tr("Abrir em nova janela"),
            Self::NewTab => crate::i18n::tr("Abrir em nova aba"),
            Self::DuplicateTab => crate::i18n::tr("Duplicar aba"),
            Self::CloseOtherTabs => crate::i18n::tr("Fechar outras abas"),
            Self::CloseRightTabs => crate::i18n::tr("Fechar abas à direita"),
            Self::Terminal => crate::i18n::tr("Abrir terminal aqui"),
            Self::OpenWith => crate::i18n::tr("Abrir com outro aplicativo"),
            Self::Rename => crate::i18n::tr("Renomear"),
            Self::CopyPath => crate::i18n::tr("Copiar caminho"),
            Self::Properties => crate::i18n::tr("Propriedades"),
            Self::Pin => crate::i18n::tr("Fixar pasta"),
            Self::Unpin => crate::i18n::tr("Desafixar pasta"),
            Self::Network => crate::i18n::tr("Compartilhamentos de rede"),
            Self::Trash => crate::i18n::tr("Mover para a lixeira"),
            Self::DeletePermanently => crate::i18n::tr("Excluir permanentemente"),
            Self::CancelEdit => crate::i18n::tr("Sair da edição do caminho"),
            Self::BrowseFolders => crate::i18n::tr("Navegar e selecionar pasta"),
            Self::ZoomIn => crate::i18n::tr("Aumentar tamanho dos itens"),
            Self::ZoomOut => crate::i18n::tr("Diminuir tamanho dos itens"),
            Self::TrashBin => crate::i18n::tr("Lixeira"),
            Self::Restore => crate::i18n::tr("Restaurar"),
            Self::EmptyTrash => crate::i18n::tr("Esvaziar lixeira"),
            Self::FilePdf => crate::i18n::tr("Arquivos PDF"),
            Self::FileDocument => crate::i18n::tr("Documentos Word e Writer"),
            Self::FileSpreadsheet => crate::i18n::tr("Planilhas e arquivos CSV"),
            Self::FilePresentation => crate::i18n::tr("Apresentações"),
            Self::FileArchive => crate::i18n::tr("Arquivos compactados"),
            Self::FileImage => crate::i18n::tr("Arquivos de imagem"),
            Self::FileAudio => crate::i18n::tr("Arquivos de áudio"),
            Self::FileVideo => crate::i18n::tr("Arquivos de vídeo"),
            Self::FileCode => crate::i18n::tr("Código e configurações"),
            Self::FileText => crate::i18n::tr("Texto e registros"),
            Self::FilePackage => crate::i18n::tr("Pacotes e instaladores"),
            Self::FileExecutable => crate::i18n::tr("Programas executáveis"),
            Self::FileCertificate => crate::i18n::tr("Certificados e chaves"),
            Self::FileDatabase => crate::i18n::tr("Bancos de dados"),
            Self::FileDisk => crate::i18n::tr("Imagens de disco"),
            Self::FileDiagram => crate::i18n::tr("Desenhos e diagramas"),
            Self::FileFont => crate::i18n::tr("Arquivos de fonte"),
            Self::FileBinary => crate::i18n::tr("Arquivos binários"),
            Self::FileBackup => crate::i18n::tr("Arquivos de backup"),
            Self::FileCalendar => crate::i18n::tr("Calendários e contatos"),
            Self::NoticeSuccess => crate::i18n::tr("Notificação de sucesso"),
            Self::NoticeWarning => crate::i18n::tr("Notificação de aviso"),
            Self::NoticeError => crate::i18n::tr("Notificação de erro"),
        }
    }

    pub fn default_name(self) -> &'static str {
        match self {
            Self::Home => "house",
            Self::Desktop => "desktop",
            Self::Download => "download",
            Self::Document => "file-lines",
            Self::Picture => "image",
            Self::Music => "music",
            Self::Video => "film",
            Self::Drive => "hard-drive",
            Self::Mount => "server",
            Self::Removable => "plug",
            Self::Edit => "pen-to-square",
            Self::Settings => "gear",
            Self::NewFolder => "folder-plus",
            Self::Copy => "copy",
            Self::Cut => "scissors",
            Self::Paste => "paste",
            Self::More => "ellipsis",
            Self::Hidden => "eye",
            Self::Details => "list",
            Self::List => "bars",
            Self::Grid => "table-cells-large",
            Self::Sun => "sun",
            Self::Moon => "moon",
            Self::Folder => "folder",
            Self::File => "file",
            Self::Link => "link",
            Self::Back => "arrow-left",
            Self::Forward => "arrow-right",
            Self::Up => "arrow-up",
            Self::Refresh => "arrows-rotate",
            Self::Open => "folder-open",
            Self::NewWindow => "window-restore",
            Self::NewTab => "square-plus",
            Self::DuplicateTab => "clone",
            Self::CloseOtherTabs => "rectangle-xmark",
            Self::CloseRightTabs => "right-from-bracket",
            Self::Terminal => "terminal",
            Self::OpenWith => "window-restore",
            Self::Rename => "file-pen",
            Self::CopyPath => "link",
            Self::Properties => "circle-info",
            Self::Pin => "thumbtack",
            Self::Unpin => "minus",
            Self::Network => "network-wired",
            Self::Trash => "trash-can",
            Self::DeletePermanently => "circle-xmark",
            Self::CancelEdit => "xmark",
            Self::BrowseFolders => "folder-tree",
            Self::ZoomIn => "magnifying-glass-plus",
            Self::ZoomOut => "magnifying-glass-minus",
            Self::TrashBin => "trash-can",
            Self::Restore => "trash-arrow-up",
            Self::EmptyTrash => "trash-can",
            Self::FilePdf => "file-pdf",
            Self::FileDocument => "file-word",
            Self::FileSpreadsheet => "file-excel",
            Self::FilePresentation => "file-powerpoint",
            Self::FileArchive => "file-zipper",
            Self::FileImage => "file-image",
            Self::FileAudio => "file-audio",
            Self::FileVideo => "file-video",
            Self::FileCode => "file-code",
            Self::FileText => "file-lines",
            Self::FilePackage => "box-open",
            Self::FileExecutable => "gears",
            Self::FileCertificate => "certificate",
            Self::FileDatabase => "database",
            Self::FileDisk => "compact-disc",
            Self::FileDiagram => "diagram-project",
            Self::FileFont => "font",
            Self::FileBinary => "microchip",
            Self::FileBackup => "box-archive",
            Self::FileCalendar => "calendar-days",
            Self::NoticeSuccess => "circle-check",
            Self::NoticeWarning => "triangle-exclamation",
            Self::NoticeError => "circle-xmark",
        }
    }

    fn color(self, dark: bool) -> Color32 {
        let color = match self {
            Self::Folder | Self::NewFolder => Color32::from_rgb(205, 146, 24),
            Self::Music | Self::Video => Color32::from_rgb(150, 85, 194),
            Self::Picture | Self::Download => Color32::from_rgb(28, 148, 105),
            Self::Settings | Self::Drive | Self::Mount | Self::Removable => {
                Color32::from_rgb(95, 120, 145)
            }
            Self::Sun => Color32::from_rgb(210, 143, 25),
            Self::FilePdf => Color32::from_rgb(201, 46, 42),
            Self::FileDocument => Color32::from_rgb(40, 92, 165),
            Self::FileSpreadsheet => Color32::from_rgb(33, 126, 76),
            Self::FilePresentation => Color32::from_rgb(202, 81, 44),
            Self::FileArchive | Self::FileCertificate => Color32::from_rgb(177, 129, 25),
            Self::FileImage => Color32::from_rgb(28, 148, 105),
            Self::FileAudio | Self::FileVideo | Self::FileFont => Color32::from_rgb(150, 85, 194),
            Self::FileCode | Self::FileDiagram | Self::FileDatabase => {
                Color32::from_rgb(29, 132, 156)
            }
            Self::FileText | Self::FileBinary | Self::FileDisk | Self::FileExecutable => {
                Color32::from_rgb(95, 120, 145)
            }
            Self::FilePackage | Self::FileBackup => Color32::from_rgb(160, 110, 57),
            Self::FileCalendar => Color32::from_rgb(198, 108, 35),
            Self::NoticeSuccess => Color32::from_rgb(21, 128, 61),
            Self::NoticeWarning => Color32::from_rgb(146, 92, 0),
            Self::NoticeError => Color32::from_rgb(185, 28, 28),
            _ => Color32::from_rgb(30, 119, 200),
        };
        if dark {
            color.lerp_to_gamma(Color32::WHITE, 0.22)
        } else {
            color
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IconMode {
    #[default]
    Monochrome,
    Colored,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub mode: IconMode,
    pub size: f32,
    pub overrides: BTreeMap<Icon, String>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: IconMode::Monochrome,
            size: 18.0,
            overrides: BTreeMap::new(),
        }
    }
}

impl Appearance {
    pub fn name(&self, icon: Icon) -> &str {
        self.overrides
            .get(&icon)
            .map(String::as_str)
            .unwrap_or_else(|| icon.default_name())
    }

    pub fn normalize(&mut self) {
        self.size = if self.size.is_finite() {
            self.size.clamp(16.0, 28.0)
        } else {
            18.0
        };
        self.overrides.retain(|_, name| glyph(name).is_some());
    }
}

#[derive(Deserialize)]
pub struct Glyph {
    pub name: String,
    pub label: String,
    unicode: String,
}

impl Glyph {
    pub fn character(&self) -> char {
        u32::from_str_radix(&self.unicode, 16)
            .ok()
            .and_then(char::from_u32)
            .unwrap_or('?')
    }
}

pub fn catalog() -> &'static [Glyph] {
    static CATALOG: OnceLock<Vec<Glyph>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/fontawesome/catalog.json"))
            .expect("catálogo Font Awesome incorporado inválido")
    })
}

pub fn glyph(name: &str) -> Option<&'static Glyph> {
    let catalog = catalog();
    catalog
        .binary_search_by(|g| g.name.as_str().cmp(name))
        .ok()
        .map(|i| &catalog[i])
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "fontawesome".into(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fontawesome/fa-solid-900.otf"
        ))),
    );
    fonts.families.insert(
        FontFamily::Name("fontawesome".into()),
        vec!["fontawesome".into()],
    );
    ctx.set_fonts(fonts);
}

pub fn paint_glyph(painter: &egui::Painter, rect: Rect, glyph: &Glyph, color: Color32) {
    painter.with_clip_rect(rect.expand(1.0)).text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph.character(),
        FontId::new(
            rect.height().min(rect.width()),
            FontFamily::Name("fontawesome".into()),
        ),
        color,
    );
}

pub fn paint(
    painter: &egui::Painter,
    rect: Rect,
    icon: Icon,
    color: Color32,
    appearance: &Appearance,
) {
    let color = if appearance.mode == IconMode::Colored {
        icon.color(painter.ctx().style().visuals.dark_mode)
    } else {
        color
    };
    let glyph = glyph(appearance.name(icon))
        .or_else(|| glyph(icon.default_name()))
        .expect("ícone padrão ausente");
    paint_glyph(painter, rect, glyph, color);
}

pub fn button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    appearance: &Appearance,
) -> egui::Response {
    selectable_button(ui, icon, label, false, appearance)
}

pub fn selectable_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    appearance: &Appearance,
) -> egui::Response {
    let response = ui.add(egui::Button::new("").selected(selected).min_size(Vec2::new(
        (appearance.size + 14.0).max(32.0),
        (appearance.size + 10.0).max(28.0),
    )));
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
    });
    paint(
        ui.painter(),
        Rect::from_center_size(response.rect.center(), Vec2::splat(appearance.size)),
        icon,
        if selected {
            ui.visuals().selection.stroke.color
        } else {
            ui.visuals().text_color()
        },
        appearance,
    );
    response.on_hover_text(label)
}

pub fn menu_item(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    shortcut: &str,
    enabled: bool,
    selected: bool,
    appearance: &Appearance,
) -> egui::Response {
    use egui::AtomExt;
    let glyph = glyph(appearance.name(icon))
        .or_else(|| glyph(icon.default_name()))
        .expect("ícone padrão ausente");
    let color = if appearance.mode == IconMode::Colored {
        icon.color(ui.visuals().dark_mode)
    } else {
        ui.visuals().text_color()
    };
    let symbol = egui::RichText::new(glyph.character().to_string())
        .font(FontId::new(
            appearance.size,
            FontFamily::Name("fontawesome".into()),
        ))
        .color(color)
        .atom_size(Vec2::splat(appearance.size));
    let button = egui::Button::new((symbol, label.atom_shrink(true)))
        .truncate()
        .shortcut_text(shortcut)
        .selected(selected)
        .min_size(Vec2::new(0.0, (appearance.size + 10.0).max(30.0)));
    let response = ui.add_enabled(enabled, button);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });
    response
}
