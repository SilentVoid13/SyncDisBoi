use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicResponse {
    pub contents: ResponseContent,
}
impl YtMusicResponse {
    pub fn merge(&mut self, other: &mut YtMusicContinuationResponse) -> Option<()> {
        if let Some(isrc) = self.get_item_section_renderer_content() {
            Some(
                isrc.grid_renderer.as_mut()?.items.append(
                    &mut other
                        .continuation_contents
                        .grid_continuation
                        .as_mut()?
                        .items,
                ),
            )
        } else if let Some(mpsr) = self.get_music_playlist_shelf_renderer() {
            Some(
                mpsr.contents.as_mut()?.append(
                    other
                        .continuation_contents
                        .music_playlist_shelf_continuation
                        .as_mut()?
                        .contents
                        .as_mut()?
                ),
            )
        } else {
            None
        }
    }

    pub fn get_mrlirs(&mut self) -> Option<Vec<&MusicResponsiveListItemRenderer>> {
        Some(
            self.get_section_renderer_content()?
                .music_playlist_shelf_renderer
                .as_ref()?
                .contents
                .as_ref()?
                .iter()
                .map(|item| &item.music_responsive_list_item_renderer)
                .collect(),
        )
    }

    pub fn get_mtrirs(&mut self) -> Option<Vec<&MusicTwoRowItemRenderer>> {
        Some(
            self.get_item_section_renderer_content()?
                .grid_renderer
                .as_ref()?
                .items
                .iter()
                .map(|item2| &item2.music_two_row_item_renderer)
                .collect(),
        )
    }

    pub fn get_section_renderer_content(&mut self) -> Option<&mut SectionRendererContent> {
        self.contents.single_column_browse_results_renderer.tabs[0]
            .tab_renderer
            .content
            .section_list_renderer
            .contents
            .iter_mut()
            .find(|item| {
                item.music_playlist_shelf_renderer.is_some() || item.item_section_renderer.is_some()
            })
    }

    pub fn get_item_section_renderer_content(&mut self) -> Option<&mut ItemSectionRendererContent> {
        Some(
            &mut self
                .get_section_renderer_content()?
                .item_section_renderer
                .as_mut()?
                .contents[0],
        )
    }

    pub fn get_music_playlist_shelf_renderer(&mut self) -> Option<&mut MusicPlaylistShelfRenderer> {
        Some(
            self.get_section_renderer_content()?
                .music_playlist_shelf_renderer
                .as_mut()?,
        )
    }

    pub fn get_continuation(&mut self) -> Option<String> {
        if let Some(isrc) = self.get_item_section_renderer_content() {
            Some(isrc.grid_renderer.as_ref()?.continuations.as_ref()?[0].get_continuation())
        } else if let Some(mpsr) = self.get_music_playlist_shelf_renderer() {
            Some(mpsr.continuations.as_ref()?[0].get_continuation())
        } else {
            None
        }
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentsVec<T> {
    pub contents: Vec<T>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentsSingle<T> {
    pub contents: [T; 1],
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Content<T> {
    pub content: T,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResponseContent {
    pub single_column_browse_results_renderer: SingleColumnBrowseResultsRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SingleColumnBrowseResultsRenderer {
    pub tabs: [Tab; 1],
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub tab_renderer: Content<TabRendererContent>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TabRendererContent {
    pub section_list_renderer: ContentsVec<SectionRendererContent>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SectionRendererContent {
    pub music_playlist_shelf_renderer: Option<MusicPlaylistShelfRenderer>,
    pub item_section_renderer: Option<ContentsSingle<ItemSectionRendererContent>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylistShelfRenderer {
    pub contents: Option<Vec<MusicPlaylistShelfRendererContent>>,
    pub continuations: Option<[Continuation; 1]>,
}
impl MusicPlaylistShelfRenderer {
    pub fn get_continuation(&self) -> Option<String> {
        Some(self.continuations.as_ref()?[0].get_continuation())
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylistShelfRendererContent {
    pub music_responsive_list_item_renderer: MusicResponsiveListItemRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveListItemRenderer {
    pub menu: Option<Menu>,
    pub overlay: Option<Overlay>,
    pub flex_columns: [FlexColumn; 3],
    pub playlist_item_data: Option<PlaylistItemData>,
}
impl MusicResponsiveListItemRenderer {
    pub fn get_set_id(&self) -> Option<String> {
        Some(
            self.playlist_item_data
                .as_ref()?
                .playlist_set_video_id
                .clone(),
        )
    }

    pub fn get_id(&self) -> Option<String> {
        Some(self.playlist_item_data.as_ref()?.video_id.clone())
    }

    pub fn get_flex_run_text(&self, flex_i: usize, run_i: usize) -> Option<String> {
        Some(self.get_flex_runs(flex_i)?.get(run_i)?.get_text())
    }

    pub fn get_flex_run_id(&self, flex_i: usize, run_i: usize) -> Option<String> {
        Some(self.get_flex_runs(flex_i)?.get(run_i)?.get_id()?)
    }

    pub fn get_flex_runs(&self, flex_i: usize) -> Option<&Vec<Run>> {
        Some(
            self.flex_columns
                .get(flex_i)?
                .music_responsive_list_item_flex_column_renderer
                .text
                .runs
                .as_ref()?,
        )
    }

    #[allow(dead_code)]
    fn get_action(&self) -> Option<&Action> {
        Some(
            &self
                .menu
                .as_ref()?
                .menu_renderer
                .items
                .iter()
                .filter_map(|item| {
                    Some(&item.menu_service_item_renderer.as_ref()?.service_endpoint)
                })
                .find(|item| item.playlist_edit_endpoint.is_some())?
                .playlist_edit_endpoint
                .as_ref()?
                .actions[0],
        )
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ItemSectionRendererContent {
    pub grid_renderer: Option<GridRenderer>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GridRenderer {
    pub items: Vec<Item2>,
    pub continuations: Option<[Continuation; 1]>,
}
impl GridRenderer {
    pub fn get_continuation(&self) -> Option<String> {
        Some(self.continuations.as_ref()?[0].get_continuation())
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item2 {
    pub music_two_row_item_renderer: MusicTwoRowItemRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicTwoRowItemRenderer {
    pub menu: Option<Menu>,
    pub title: Text,
}
impl MusicTwoRowItemRenderer {
    pub fn get_id(&self) -> Option<String> {
        Some(self.title.runs.as_ref()?.get(0)?.get_id()?)
    }

    pub fn get_name(&self) -> Option<String> {
        Some(self.title.runs.as_ref()?.get(0)?.get_text())
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Continuation {
    pub next_continuation_data: NextContinuationData,
}
impl Continuation {
    pub fn get_continuation(&self) -> String {
        return self.next_continuation_data.continuation.clone();
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NextContinuationData {
    pub continuation: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FlexColumn {
    pub music_responsive_list_item_flex_column_renderer: MusicResponsiveListItemFlexColumnRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveListItemFlexColumnRenderer {
    pub text: Text,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Text {
    pub runs: Option<Vec<Run>>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub text: String,
    pub navigation_endpoint: Option<NavigationEndpoint>,
}
impl Run {
    pub fn get_text(&self) -> String {
        self.text.clone()
    }

    pub fn get_id(&self) -> Option<String> {
        Some(
            self.navigation_endpoint
                .as_ref()?
                .browse_endpoint
                .as_ref()?
                .browse_id
                .clone(),
        )
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NavigationEndpoint {
    pub browse_endpoint: Option<BrowseEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpoint {
    pub browse_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Menu {
    pub menu_renderer: MenuRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MenuRenderer {
    pub items: Vec<Item>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub menu_service_item_renderer: Option<MenuServiceItemRenderer>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MenuServiceItemRenderer {
    pub service_endpoint: ServiceEndpoint,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceEndpoint {
    pub playlist_edit_endpoint: Option<PlaylistEditEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEditEndpoint {
    pub actions: [Action; 1],
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub action: String,
    pub removed_video_id: String,
    pub set_video_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Overlay {
    pub music_item_thumbnail_overlay_renderer: Content<MusicItemThumbnailOverlayRendererContent>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicItemThumbnailOverlayRendererContent {
    pub music_play_button_renderer: MusicPlayButtonRenderer,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlayButtonRenderer {
    pub play_navigation_endpoint: Option<PlayNavigationEndpoint>,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlayNavigationEndpoint {
    pub watch_endpoint: WatchEndpoint,
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WatchEndpoint {
    pub playlist_id: String,
    pub video_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItemData {
    pub playlist_set_video_id: String,
    pub video_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct YtMusicContinuationResponse {
    pub continuation_contents: ContinuationContents,
}
impl YtMusicContinuationResponse {
    pub fn get_continuation(&self) -> Option<String> {
        if let Some(g) = &self.continuation_contents.grid_continuation {
            g.get_continuation()
        } else if let Some(m) = &self.continuation_contents.music_playlist_shelf_continuation {
            m.get_continuation()
        } else { None }
    }
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationContents {
    pub grid_continuation: Option<GridRenderer>,
    pub music_playlist_shelf_continuation: Option<MusicPlaylistShelfRenderer>,
}
