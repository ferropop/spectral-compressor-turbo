#pragma push_macro("main")
#define main previous_auto_main
#include "auto_gain_host.cpp"
#pragma pop_macro("main")
struct Result {std::vector<float> audio;std::vector<double> gain;int latency;};
static float drum(int64_t n,int c,double rate,double period) {
    if(n<0)return 0;
    double t=n/rate, beat=std::fmod(t,0.5),phase=std::fmod(t,period);
    uint32_t h=uint32_t(n%int64_t(rate*period))*747796405u+uint32_t(c+1)*2891336453u;h=((h>>((h>>28)+4))^h)*277803737u;h=(h>>22)^h;
    double noise=double(h)/4294967296.0-.5;
    double kick=.65*std::exp(-beat/.055)*std::sin(2*M_PI*(52*beat+1.2*(1-std::exp(-beat/.015))));
    double snare=phase>period*.5?.16*noise*std::exp(-(phase-period*.5)/.08):0;
    double hat=.025*noise*std::exp(-std::fmod(t,.125)/.015);
    return float(kick+snare+hat+.02*std::sin(2*M_PI*(c?730:410)*t));
}
static Result render(const char* binary,int rate,int block,int profile,double mix,double gain,bool delta,bool agc,bool smart,double seconds,double period=0) {
    Plugin p(binary);p.setup(rate,block,2,profile);p.plain("Mix",mix);p.plain("Output Gain",gain);
    Changes initial;
    for(auto pair:std::vector<std::pair<std::string,double>>{{"Mix",mix},{"Output Gain",gain},{"Delta",delta?1.0:0.0},{"Auto Gain Compensation",agc?1.0:0.0},{"Smart Gain Averaging",smart?1.0:0.0}})automate(p,initial,pair.first,pair.second);
    Result r;r.latency=p.processor->getLatencySamples();Packet packet(block);Changes none;
    int total=int(rate*seconds);
    for(int offset=0;offset<total;offset+=block) {
        int count=std::min(block,total-offset);
        for(int c=0;c<2;c++)for(int i=0;i<count;i++) {
            packet.input[c][i]=period?drum(offset+i,c,rate,period):signal(offset+i,c,rate);
            packet.side[c][i]=signal(offset+i,c,rate,true);packet.output[c][i]=NAN;
        }
        ProcessData d;d.processMode=kOffline;d.symbolicSampleSize=kSample32;d.numSamples=count;d.numInputs=2;d.numOutputs=1;d.inputs=packet.ins;d.outputs=packet.outs;d.inputParameterChanges=offset==0?&initial:&none;
        check(p.processor->process(d),"process");
        for(int i=0;i<count;i++)for(int c=0;c<2;c++) {if(!std::isfinite(packet.output[c][i]))throw std::runtime_error("nonfinite");r.audio.push_back(packet.output[c][i]);}
        r.gain.push_back(gainDb(p));
    }
    return r;
}
int main(int argc,char** argv) {
    try {
        if(argc!=3)throw std::runtime_error("Usage: turbo_features binary report-folder");
        std::filesystem::path folder(argv[2]);std::filesystem::create_directories(folder);int cases=0;double worst=0;
        for(int rate:{44100,48000,96000})for(int block:{64,257,1024})for(int profile:{0,2})for(double mix:{0.,.5,1.})for(double gain:{.5,2.}) {
            auto normal=render(argv[1],rate,block,profile,mix,gain,false,false,true,1.2);
            auto delta=render(argv[1],rate,block,profile,mix,gain,true,false,true,1.2);
            double error=0,peak=0;
            for(size_t n=normal.latency+block+1024;n<normal.audio.size()/2;n++)for(int c=0;c<2;c++) {
                double expected=gain*signal(int64_t(n)-normal.latency,c,rate)-normal.audio[2*n+c];
                error=std::max(error,std::abs(expected-delta.audio[2*n+c]));peak=std::max(peak,double(std::abs(delta.audio[2*n+c])));
            }
            if(error>2e-6){std::cerr<<"rate="<<rate<<" block="<<block<<" profile="<<profile<<" mix="<<mix<<" gain="<<gain<<" latency="<<normal.latency<<" error="<<error<<"\n";throw std::runtime_error("Delta did not equal latency-aligned dry minus pre-gain processing");}
            if(mix==0 && peak!=0)throw std::runtime_error("Delta at dry mix must be exact silence");
            if(profile==2 && mix>0 && peak<0.001)throw std::runtime_error("Active processing produced no audible Delta");
            worst=std::max(worst,error);++cases;
        }
        std::cout<<"DELTA PASS cases="<<cases<<" max_sample_error="<<worst<<" dry_mix_exact_silence=true\n";
        auto normal=render(argv[1],48000,257,2,1,1,false,true,true,8.);
        auto delta=render(argv[1],48000,257,2,1,1,true,true,true,8.);
        if(normal.gain!=delta.gain)throw std::runtime_error("AGC was influenced by Delta monitoring");
        std::cout<<"DELTA_AGC_INDEPENDENCE PASS blocks="<<normal.gain.size()<<" gain_traces_bit_identical=true\n";
        std::ofstream report(folder/"drum-pumping.tsv");report<<"rate\tblock\tprofile\tperiod\tsmart_range_db\tmomentary_range_db\n";
        int drumcases=0;
        for(int profile:{1,2})for(double period:{1.,2.}) {
            int rate=48000,block=257;
            auto s=render(argv[1],rate,block,profile,1,1,false,true,true,16.,period);
            auto f=render(argv[1],rate,block,profile,1,1,false,true,false,16.,period);
            wav(folder/("smart-profile"+std::to_string(profile)+"-loop"+std::to_string(int(period))+".wav"),s.audio,rate,2);
            wav(folder/("momentary-profile"+std::to_string(profile)+"-loop"+std::to_string(int(period))+".wav"),f.audio,rate,2);
            std::vector<float> input;for(int n=0;n<rate*16;n++)for(int c=0;c<2;c++)input.push_back(drum(n-s.latency,c,rate,period));
            wav(folder/("reference-profile"+std::to_string(profile)+"-loop"+std::to_string(int(period))+".wav"),input,rate,2);
            auto range=[&](const Result& r) {size_t start=size_t(rate*10/block);auto mm=std::minmax_element(r.gain.begin()+start,r.gain.end());return *mm.second-*mm.first;};
            double sr=range(s),fr=range(f);
            report<<rate<<"\t"<<block<<"\t"<<profile<<"\t"<<period<<"\t"<<sr<<"\t"<<fr<<"\n";
            std::cout<<"DRUM profile="<<profile<<" loop="<<period<<"s smart="<<sr<<"dB momentary="<<fr<<"dB\n";
            if(sr>0.7 || sr>fr*.5)throw std::runtime_error("Smart gain did not sufficiently reduce drum pumping");
            std::ofstream trace(folder/("gain-profile"+std::to_string(profile)+"-loop"+std::to_string(int(period))+".csv"));trace<<"seconds,smart_db,momentary_db\n";
            for(size_t i=0;i<s.gain.size();i++)trace<<double((i+1)*block)/rate<<","<<s.gain[i]<<","<<f.gain[i]<<"\n";
            ++drumcases;
        }
        std::cout<<"DRUM SMART PASS cases="<<drumcases<<"\n";return 0;
    }catch(const std::exception& e){std::cerr<<"ERROR: "<<e.what()<<"\n";return 1;}
}
